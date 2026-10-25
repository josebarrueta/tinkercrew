//! Native-only setup hardware evidence; observations are not persisted control authority.
use crate::{
    connection::{self, BoardProfile, Connections},
    management, pairing, session,
    setup::{self, HardwareObservation, SetupObservation, SetupStore},
};
mod inputs;
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

struct HardwareSession {
    observation: SetupObservation,
    profile: BoardProfile,
    client: Client,
    auth: session::GameplayAuth,
    forward: pairing::Forward,
    test_id: String,
    started: Instant,
    inputs: Option<inputs::InputWitness>,
}
#[derive(Default)]
pub struct SetupHardware(Mutex<Option<HardwareSession>>);

fn monotonic_ms() -> Result<u64, String> {
    let mut value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut value) } != 0
        || value.tv_sec < 0
        || value.tv_nsec < 0
    {
        return Err("Cannot establish a fresh monotonic hardware witness.".into());
    }
    (value.tv_sec as u64)
        .checked_mul(1000)
        .and_then(|ms| ms.checked_add(value.tv_nsec as u64 / 1_000_000))
        .ok_or("Invalid monotonic hardware time.".into())
}
fn hardware_witness(test_id: &str) -> Result<HardwareObservation, String> {
    Ok(HardwareObservation {
        test_id: test_id.into(),
        mac_boot: crate::recovery::boot_session()?.to_ascii_lowercase(),
        monotonic_ms: monotonic_ms()?,
    })
}
fn require_fresh_witness(observed: &SetupObservation) -> Result<(), String> {
    let witness = observed
        .hardware
        .as_ref()
        .ok_or("A fresh hardware self-test is required.")?;
    if witness.mac_boot != crate::recovery::boot_session()?.to_ascii_lowercase()
        || monotonic_ms()?
            .checked_sub(witness.monotonic_ms)
            .is_none_or(|age| age > 120000)
    {
        return Err("The hardware witness is stale or belongs to an earlier Mac boot. Repeat the self-test; historical checkpoints never grant live arming.".into());
    }
    Ok(())
}
fn validate_hello(
    hello: &Value,
    observed: &SetupObservation,
    same_process: bool,
) -> Result<(), String> {
    if hello["protocol"] != "1.0"
        || hello["board"]["board_id"].as_str() != Some(&observed.board_id)
        || hello["runtime"]["boot_id"].as_str() != Some(&observed.boot_id)
        || hello["runtime"]["release"].as_str() != Some(&observed.runtime_release)
        || hello["runtime"]["nonce"]
            .as_str()
            .is_none_or(|value| value.len() != 22)
        || (same_process && hello["runtime"]["nonce"].as_str() != Some(&observed.runtime_nonce))
    {
        return Err("The verified Board boot/runtime/process changed. Repeat the current setup proof; no stale hardware evidence was accepted.".into());
    }
    if ["buttons", "distance", "pixels", "buzzer", "matrix"]
        .iter()
        .any(|name| hello["capabilities"][name]["health"] != "operational")
    {
        return Err("Required hardware is missing, stale or degraded. Check power/wiring before retrying; this is not a LEGO-design failure.".into());
    }
    Ok(())
}
fn fresh_inputs(state: &Value) -> Result<f64, String> {
    inputs::fresh_distance(state)
}
async fn open(
    connections: &Connections,
    observed: &SetupObservation,
) -> Result<
    (
        BoardProfile,
        Client,
        session::GameplayAuth,
        pairing::Forward,
        Value,
    ),
    String,
> {
    let profile = connections
        .profile(&observed.board_id)
        .map_err(|error| error.message)?;
    if profile.installation_id.is_none() || profile.adb_serial != observed.serial {
        return Err("Select the same registered USB Board used by the guide.".into());
    }
    let serial = observed.serial.clone();
    let (host, forward) = tauri::async_runtime::spawn_blocking(move || {
        Ok::<_, String>((
            management::inspect(&serial)?,
            pairing::Forward::new(&serial)?,
        ))
    })
    .await
    .map_err(|_| "Hardware USB proof stopped.")??;
    if host.serial != observed.soc || host.boot_id.as_deref() != Some(&observed.boot_id) {
        return Err("The physical Board/kernel changed; reverify the guide's restart step.".into());
    }
    let candidate =
        connection::Candidate::new("127.0.0.1", forward.port).map_err(|error| error.message)?;
    let (client, _) = connection::secure_client(&profile, &candidate)
        .await
        .map_err(|error| error.message)?;
    let auth = session::GameplayAuth::from_profile(&profile, &connections.directory)
        .map_err(|error| error.message)?;
    let hello = match connection::request(
        &client,
        &profile,
        forward.port,
        Method::GET,
        "/v1/hello",
        None,
        &auth,
    )
    .await
    {
        Ok(hello) => hello,
        Err(error) => {
            auth.close(&client, &profile.tls_name, forward.port).await;
            return Err(error.message);
        }
    };
    Ok((profile, client, auth, forward, hello))
}
async fn call(
    context: &HardwareSession,
    method: Method,
    route: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    connection::request(
        &context.client,
        &context.profile,
        context.forward.port,
        method,
        route,
        body,
        &context.auth,
    )
    .await
    .map_err(|error| error.message)
}
async fn close(context: HardwareSession) {
    context
        .auth
        .close(
            &context.client,
            &context.profile.tls_name,
            context.forward.port,
        )
        .await;
}
async fn poll(context: &mut HardwareSession) -> Result<Value, String> {
    if context.started.elapsed() > Duration::from_secs(90) {
        return Err("The hardware witness expired. Start a fresh self-test.".into());
    }
    let hello = call(context, Method::GET, "/v1/hello", None).await?;
    validate_hello(&hello, &context.observation, true)?;
    let state = call(context, Method::GET, "/v1/state", None).await?;
    let after = context
        .inputs
        .as_ref()
        .ok_or("Start a fresh input witness first.")?
        .after();
    let events = call(
        context,
        Method::GET,
        &format!("/v1/events?after={after}"),
        None,
    )
    .await?;
    let inputs = context
        .inputs
        .as_mut()
        .ok_or("Start a fresh input witness first.")?;
    inputs.observe(&state, &events)?;
    if context.started.elapsed() > Duration::from_secs(90) {
        return Err(
            "The hardware witness expired during observation. Start a fresh self-test.".into(),
        );
    }
    Ok(
        json!({"state":"awaiting_confirmation","buttons":inputs.buttons(),"distanceMoved":inputs.moved(),"distanceMillimeters":inputs.distance(),"remainingMs":90000u128.saturating_sub(context.started.elapsed().as_millis())}),
    )
}

#[tauri::command]
pub async fn start_setup_hardware(
    connections: tauri::State<'_, Connections>,
    store: tauri::State<'_, SetupStore>,
    hardware: tauri::State<'_, SetupHardware>,
    board_id: String,
) -> Result<Value, String> {
    let _permit = connections.begin_maintenance()?;
    connection::disconnect_board(connections.clone())
        .await
        .map_err(|error| error.message)?;
    let mut observation = setup::prepare_hardware_step(&store, &board_id)?;
    if let Some(previous) = hardware.0.lock().await.take() {
        close(previous).await;
    }
    let (profile, client, auth, forward, hello) = open(&connections, &observation).await?;
    if let Err(error) = validate_hello(&hello, &observation, false) {
        auth.close(&client, &profile.tls_name, forward.port).await;
        return Err(error);
    }
    if hello["health"]["safe_state"] != true {
        auth.close(&client, &profile.tls_name, forward.port).await;
        return Err("Stop other gameplay before starting the setup self-test.".into());
    }
    observation.runtime_nonce = hello["runtime"]["nonce"].as_str().unwrap().into();
    let mut context = HardwareSession {
        observation,
        profile,
        client,
        auth,
        forward,
        test_id: String::new(),
        started: Instant::now(),
        inputs: None,
    };
    let result = async {
        let state=call(&context,Method::GET,"/v1/state",None).await?;
        let events = call(&context, Method::GET, "/v1/events?after=0", None).await?;
        context.inputs=Some(inputs::InputWitness::new(&state,&events)?);
        call(&context, Method::POST, "/v1/control-lease", Some(json!({"lease_ms":1500}))).await?;
        let test = call(&context, Method::POST, "/v1/self-tests", Some(json!({}))).await?;
        context.test_id = test["id"].as_str().filter(|id| session::valid_id(id)).ok_or("Invalid bounded self-test identity.")?.into();
        if test["state"] != "awaiting_confirmation" { return Err("The Board did not begin the bounded self-test.".into()); }
        Ok::<_, String>(json!({"state":"awaiting_confirmation","testId":context.test_id,"buttons":[false,false,false],"distanceMoved":false,"message":"Observe the bounded lights/sound, move the distance input and press A, B and C. This does not arm a Mission."}))
    }.await;
    match result {
        Ok(view) => {
            *hardware.0.lock().await = Some(context);
            Ok(view)
        }
        Err(error) => {
            close(context).await;
            Err(error)
        }
    }
}
#[tauri::command]
pub async fn poll_setup_hardware(
    hardware: tauri::State<'_, SetupHardware>,
    board_id: String,
) -> Result<Value, String> {
    let mut slot = hardware.0.lock().await;
    let context = slot
        .as_mut()
        .filter(|context| context.observation.board_id == board_id)
        .ok_or("Start a fresh self-test for this Board first.")?;
    let result = poll(context).await;
    if result.is_err()
        && let Some(context) = slot.take()
    {
        close(context).await;
    }
    result
}
#[tauri::command]
pub async fn finish_setup_hardware(
    connections: tauri::State<'_, Connections>,
    store: tauri::State<'_, SetupStore>,
    hardware: tauri::State<'_, SetupHardware>,
    board_id: String,
    lights: bool,
    sound: bool,
) -> Result<Value, String> {
    let _permit = connections.begin_maintenance()?;
    let mut slot = hardware.0.lock().await;
    if slot
        .as_ref()
        .is_none_or(|context| context.observation.board_id != board_id)
    {
        return Err("Start a fresh self-test for this Board first.".into());
    }
    let mut context = slot.take().ok_or("Start a fresh self-test first.")?;
    let result = async {
        poll(&mut context).await?;
        if context.inputs.as_ref().is_none_or(|inputs| !inputs.ready()) || !lights || !sound { return Err("Fresh movement, all three button presses and explicit light/sound confirmation are required. Start again if the witness expired.".into()); }
        // Confirmation may occur after the bounded lease expired. Explicitly acquire
        // only our own lease; another live controller cannot be displaced.
        call(&context, Method::POST, "/v1/control-lease", Some(json!({"lease_ms":1000}))).await?;
        let confirmation = call(&context, Method::POST, &format!("/v1/self-tests/{}/confirmation",context.test_id), Some(json!({"lights":lights,"sound":sound}))).await?;
        if confirmation["id"].as_str() != Some(&context.test_id) || confirmation["state"] != "passed" { return Err("The actual self-test did not pass; no setup success was recorded.".into()); }
        let hello = call(&context, Method::GET, "/v1/hello", None).await?;
        validate_hello(&hello, &context.observation, true)?;
        if hello["health"]["safe_state"] != true { return Err("Disarmed state was not observed after the self-test.".into()); }
        if context.started.elapsed() > Duration::from_secs(90) { return Err("The hardware witness expired during confirmation; retest before recording it.".into()); }
        context.observation.hardware = Some(hardware_witness(&context.test_id)?);
        setup::observed_step(&store,10,context.observation.clone())?;
        Ok::<_, String>(json!({"state":"hardware_observed","message":"Fresh input events and the bounded self-test result were observed; light/sound were explicitly human-confirmed. Outputs are disarmed. This is not a physical safe-off timing measurement."}))
    }.await;
    close(context).await;
    result
}
#[tauri::command]
pub async fn cancel_setup_hardware(
    hardware: tauri::State<'_, SetupHardware>,
) -> Result<(), String> {
    if let Some(context) = hardware.0.lock().await.take() {
        close(context).await;
    }
    Ok(())
}
#[tauri::command]
pub async fn finish_setup_crew(
    connections: tauri::State<'_, Connections>,
    store: tauri::State<'_, SetupStore>,
    progress: tauri::State<'_, crate::progress::ProgressStore>,
    board_id: String,
    crew_confirmed: bool,
) -> Result<Value, String> {
    if !crew_confirmed {
        return Err("Explicitly confirm the saved Crew before finishing setup.".into());
    }
    let _permit = connections.begin_maintenance()?;
    connection::disconnect_board(connections.clone())
        .await
        .map_err(|error| error.message)?;
    let observed = setup::require_observed_step(&store, 11, &board_id)?;
    require_fresh_witness(&observed)?;
    let saved = progress
        .load("treasure-guardian")?
        .ok_or("Save the Crew in Treasure Guardian before finishing setup.")?;
    let data: Value = serde_json::from_str(&saved.payload)
        .map_err(|_| "Saved Crew/progress is unreadable; it was not reset.")?;
    if data["schemaVersion"] != 1
        || data["missionId"] != "treasure-guardian"
        || data["missionVersion"] != 1
        || data["stageIndex"].as_u64().is_none()
    {
        return Err("Saved Mission/Crew progress is incompatible; it was not replaced or treated as setup evidence.".into());
    }
    let crew = data["crew"]
        .as_array()
        .filter(|crew| crew.len() == 3)
        .ok_or("Save three Crew members and their roles/tracks first.")?;
    let mut roles = std::collections::BTreeSet::new();
    for member in crew {
        let name = member["name"].as_str().filter(|name| {
            !name.trim().is_empty()
                && name.chars().count() <= 60
                && !name.chars().any(char::is_control)
        });
        let role = member["role"]
            .as_str()
            .filter(|role| ["Storykeeper", "Builder", "Logic keeper"].contains(role))
            .ok_or("Invalid saved Crew role.")?;
        if name.is_none()
            || ![Some("Explorer"), Some("Inventor"), Some("Hacker")]
                .contains(&member["track"].as_str())
            || !roles.insert(role)
        {
            return Err("Saved Crew names/roles/tracks are incomplete.".into());
        }
    }
    let (profile, client, auth, forward, hello) = open(&connections, &observed).await?;
    let result = async {
        validate_hello(&hello,&observed,true)?;
        if hello["health"]["safe_state"] != true { return Err("Stop gameplay and reverify disarmed state before finishing setup.".into()); }
        fresh_inputs(&connection::request(&client,&profile,forward.port,Method::GET,"/v1/state",None,&auth).await.map_err(|error| error.message)?)?;
        require_fresh_witness(&observed)?;
        setup::observed_step(&store,11,observed)?;
        Ok::<_, String>(json!({"state":"setup_recorded","crewRevision":saved.revision,"message":"The supported hybrid setup and saved Crew were recorded with fresh observed evidence. No automatic reconnect/rearm occurred. Physical/family acceptance remains separate; gameplay requires fresh checks on reconnect."}))
    }.await;
    auth.close(&client, &profile.tls_name, forward.port).await;
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hardware_witness_canonicalizes_actual_macos_boot_identity() {
        let value = hardware_witness("00000000-0000-4000-8000-000000000003").unwrap();
        assert!(session::valid_id(&value.mac_boot));
        assert_eq!(
            value.mac_boot,
            crate::recovery::boot_session()
                .unwrap()
                .to_ascii_lowercase()
        );
        assert!(value.monotonic_ms <= monotonic_ms().unwrap());
    }
}
