use base64::{Engine, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};

fn verify(public_key: &str, signature: &str, data: &[u8]) -> Result<(), String> {
    let decode = |value: &str| -> Result<String, String> {
        String::from_utf8(
            STANDARD
                .decode(value.trim())
                .map_err(|_| "Invalid Tauri signature encoding.")?,
        )
        .map_err(|_| "Invalid signature text.".into())
    };
    let public_key =
        PublicKey::decode(&decode(public_key)?).map_err(|_| "Invalid release public key.")?;
    let signature =
        Signature::decode(&decode(signature)?).map_err(|_| "Invalid release signature.")?;
    public_key
        .verify(data, &signature, false)
        .map_err(|_| "Release signature verification failed.".into())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "Usage: tinkercrew-release-verify PUBLIC_KEY_FILE SIGNATURE_FILE ARTIFACT_FILE".into(),
        );
    }
    let key = std::fs::read_to_string(&args[0]).map_err(|_| "Could not read public key file.")?;
    let signature =
        std::fs::read_to_string(&args[1]).map_err(|_| "Could not read signature file.")?;
    let data = std::fs::read(&args[2]).map_err(|_| "Could not read release artifact.")?;
    verify(&key, &signature, &data)
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    println!("Release signature verified.");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_fixture_is_valid_and_modified_content_is_not() {
        let key = include_str!("../fixtures/public.key.txt");
        let signature = include_str!("../fixtures/manifest.json.sig");
        assert!(verify(key, signature, include_bytes!("../fixtures/manifest.json")).is_ok());
        assert!(verify(key, signature, b"modified manifest").is_err());
        assert!(verify("bad-key", signature, b"modified manifest").is_err());
    }
}
