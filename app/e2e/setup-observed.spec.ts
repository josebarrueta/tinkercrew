import { expect, test } from "@playwright/test";

// UI/IPC orchestration only. Every native operation is intercepted, including
// proof persistence. sessionStorage below belongs solely to this mock harness;
// production setup never writes proof/credentials there. No Board operation runs.
test("observed setup resumes without a client-side completion shortcut", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const scope = window as unknown as {
      isTauri: boolean;
      __proofCalls: string[];
      __allowProofInputs: boolean;
      __TAURI_INTERNALS__: {
        invoke: (
          command: string,
          args?: Record<string, unknown>,
        ) => Promise<unknown>;
      };
    };
    scope.isTauri = true;
    scope.__proofCalls = [];
    scope.__allowProofInputs = false;
    const ids = [
      "connect-usb",
      "identify",
      "boot",
      "name",
      "credentials",
      "wifi",
      "network",
      "pair",
      "deploy",
      "rediscover",
      "hardware",
      "crew",
    ];
    const initial = {
      payload: JSON.stringify({
        schemaVersion: 1,
        selectedSerial: "USB_TEST",
        stageIndex: 7,
        checkpoints: ids.slice(0, 7).map((stageId, index) => ({
          stageId,
          source: index === 0 ? "observed" : "human",
        })),
      }),
      revision: 1,
      observation: null as null | {
        serial: string;
        soc: string;
        boardId: string;
        bootId: string;
        runtimeRelease: string;
        runtimeNonce: string;
        hardware: unknown;
      },
    };
    const record = JSON.parse(
      sessionStorage.getItem("test.native.proof") ?? JSON.stringify(initial),
    ) as typeof initial;
    const persist = () =>
      sessionStorage.setItem("test.native.proof", JSON.stringify(record));
    const mark = (index: number) => {
      const progress = JSON.parse(record.payload);
      if (progress.stageIndex !== index)
        throw new Error("Mock refuses out-of-order native proofs");
      progress.stageIndex++;
      progress.checkpoints.push({
        stageId: ids[index],
        source: index === 11 ? "human" : "observed",
      });
      record.payload = JSON.stringify(progress);
      record.revision++;
      persist();
    };
    scope.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        scope.__proofCalls.push(command);
        if (command === "load_setup") return structuredClone(record);
        if (command === "list_boards") return [];
        if (command === "load_progress") {
          const payload = sessionStorage.getItem("test.native.mission");
          return payload ? { payload, revision: 1 } : null;
        }
        if (command === "save_progress") {
          sessionStorage.setItem("test.native.mission", String(args.payload));
          return Number(args.expectedRevision) + 1;
        }
        if (command === "save_setup")
          throw new Error("A frontend save cannot complete native checkpoints");
        if (command === "factory_restore_status")
          return { phase: "idle", bytes: 0, message: "Mock only" };
        if (command === "app_update_info") return { version: "0.1.0" };
        if (
          command === "initial_companion_package" ||
          command === "runtime_bundle_info"
        )
          return {
            release: "a".repeat(64),
            contract: "b".repeat(64),
            bytes: 2048,
          };
        if (command === "admin_inspect_usb" || command === "admin_login")
          return {
            adbSerial: "USB_TEST",
            serial: "42",
            username: "arduino",
            hostname: "fixture",
            os: "debian",
            osVersion: "13",
            imageVersion: "fixture",
          };
        if (command === "pair_installed_companion") {
          if (args.confirmation !== "PAIR 42")
            throw new Error("Wrong target confirmation");
          record.observation = {
            serial: "USB_TEST",
            soc: "42",
            boardId: "00000000-0000-4000-8000-000000000042",
            bootId: "00000000-0000-4000-8000-000000000001",
            runtimeRelease: "a".repeat(64),
            runtimeNonce: "n".repeat(22),
            hardware: null,
          };
          mark(7);
          return { boardId: record.observation.boardId };
        }
        if (command === "update_companion_runtime") {
          mark(8);
          return {
            message: "Mock runtime proof only; not hardware validation.",
          };
        }
        if (command === "request_verified_board_restart")
          return { state: "reboot_scheduled" };
        if (command === "verify_board_restart") {
          record.observation!.bootId = "00000000-0000-4000-8000-000000000002";
          mark(9);
          return { message: "Mock kernel/LAN/USB observation only." };
        }
        if (command === "start_setup_hardware")
          return { message: "Mock bounded self-test only." };
        if (command === "poll_setup_hardware")
          return {
            buttons: Array(3).fill(scope.__allowProofInputs),
            distanceMoved: scope.__allowProofInputs,
            distanceMillimeters: 175,
          };
        if (command === "finish_setup_hardware") {
          if (!scope.__allowProofInputs || !args.lights || !args.sound)
            throw new Error("Inputs and explicit observations required");
          record.observation!.hardware = { testId: "mock-only" };
          mark(10);
          return { message: "Mock hardware evidence only." };
        }
        if (command === "finish_setup_crew") {
          const crew = JSON.parse(
            sessionStorage.getItem("test.native.mission") ?? "{}",
          ).crew;
          if (!args.crewConfirmed || crew?.[0]?.name !== "Ada")
            throw new Error("Persisted Crew and consent required");
          mark(11);
          return {
            message:
              "Mock completion record only; physical acceptance remains unperformed.",
          };
        }
        if (
          command === "disconnect_board" ||
          command === "admin_logout" ||
          command === "cancel_setup_hardware"
        )
          return null;
        throw new Error(`Unexpected mock command: ${command}`);
      },
    };
  });
  async function openGuide() {
    await page
      .getByRole("button", { name: "☰ Board Setup", exact: true })
      .click();
    await page
      .getByText("Set up a UNO Q — resumable official-tool handoff guide", {
        exact: true,
      })
      .click();
    return page.getByRole("region", { name: "Resumable UNO Q setup guide" });
  }
  await page.goto("/");
  let guide = await openGuide();
  const proof = () =>
    guide.getByRole("region", { name: "Observed setup proofs" });
  async function inspect() {
    await proof()
      .getByRole("button", { name: "Inspect exact Board for observed setup" })
      .click();
    await proof()
      .getByRole("checkbox", {
        name: "I physically verified this selected setup Board and hardware serial",
      })
      .check();
  }
  await expect(
    guide.getByRole("heading", {
      name: "Establish full USB Pairing",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    guide.getByRole("button", { name: "Record checkpoint and continue" }),
  ).toHaveCount(0);
  await inspect();
  await proof()
    .getByRole("textbox", { name: "Type PAIR 42", exact: true })
    .fill("PAIR 42");
  await proof()
    .getByRole("button", { name: "Prove installation Pairing" })
    .click();
  await expect(
    guide.getByRole("heading", {
      name: "Verify or update the supported Companion runtime",
    }),
  ).toBeVisible();
  await proof()
    .getByRole("textbox", { name: "Type UPDATE 42", exact: true })
    .fill("UPDATE 42");
  await proof()
    .getByRole("button", { name: "Verify / update supported runtime" })
    .click();
  await expect(
    guide.getByRole("heading", { name: "Reboot and verify LAN plus USB" }),
  ).toBeVisible();
  await page.reload();
  guide = await openGuide();
  await expect(
    guide.getByRole("heading", { name: "Reboot and verify LAN plus USB" }),
  ).toBeVisible();
  await inspect();
  await proof()
    .getByLabel("Linux password for this approved setup restart")
    .fill("fixture-only-not-submitted");
  await page.getByRole("button", { name: "⌂ Home Base", exact: true }).click();
  await page
    .getByRole("button", { name: "☰ Board Setup", exact: true })
    .click();
  await expect(
    proof().getByLabel("Linux password for this approved setup restart"),
  ).toHaveValue("");
  await inspect();
  await proof()
    .getByLabel("Linux password for this approved setup restart")
    .fill("fixture-only-not-a-real-secret");
  await proof()
    .getByRole("textbox", { name: "Type RESTART 42", exact: true })
    .fill("RESTART 42");
  await proof()
    .getByRole("button", { name: "Request tracked setup restart" })
    .click();
  await expect(
    proof().getByLabel("Linux password for this approved setup restart"),
  ).toHaveValue("");
  await expect(
    guide.getByRole("heading", { name: "Reboot and verify LAN plus USB" }),
  ).toBeVisible();
  await proof().getByLabel("Current Board LAN address").fill("192.168.4.42");
  await proof()
    .getByRole("checkbox", {
      name: "Allow contact with this explicit local-network endpoint",
    })
    .check();
  await proof()
    .getByRole("button", { name: "Observe new kernel over LAN and USB" })
    .click();
  await expect(
    guide.getByRole("heading", { name: "Test the Modulinos" }),
  ).toBeVisible();
  await proof()
    .getByRole("button", { name: "Start bounded setup self-test" })
    .click();
  await proof()
    .getByRole("checkbox", { name: "We actually saw the self-test lights" })
    .check();
  await proof()
    .getByRole("checkbox", { name: "We actually heard the self-test sound" })
    .check();
  const finish = proof().getByRole("button", {
    name: "Confirm observed setup self-test",
  });
  await expect(finish).toBeDisabled();
  await page.evaluate(() => {
    (window as unknown as { __allowProofInputs: boolean }).__allowProofInputs =
      true;
  });
  await expect(finish).toBeEnabled();
  await finish.click();
  await expect(
    guide.getByRole("heading", { name: "Create the Crew and begin" }),
  ).toBeVisible();
  await proof().getByLabel("Storykeeper name").fill("Ada");
  await proof()
    .getByRole("checkbox", {
      name: "We confirm this saved Crew and are ready to begin",
    })
    .check();
  await proof()
    .getByRole("button", { name: "Save Crew and record setup completion" })
    .click();
  await expect(
    guide.getByRole("heading", { name: "Setup recorded", exact: true }),
  ).toBeVisible();
  await expect(
    proof()
      .getByRole("status")
      .filter({ hasText: "Supported hybrid setup was recorded." }),
  ).toContainText("does not prove current wiring");
  const result = await page.evaluate(() => ({
    calls: (window as unknown as { __proofCalls: string[] }).__proofCalls,
    record: JSON.parse(sessionStorage.getItem("test.native.proof")!),
    mission: sessionStorage.getItem("test.native.mission"),
  }));
  expect(JSON.parse(result.record.payload).stageIndex).toBe(12);
  expect(result.calls).not.toContain("save_setup");
  expect(result.calls.indexOf("save_progress")).toBeLessThan(
    result.calls.indexOf("finish_setup_crew"),
  );
  expect(JSON.stringify(result.record)).not.toContain(
    "fixture-only-not-a-real-secret",
  );
  expect(result.mission).toContain("Ada");
});
