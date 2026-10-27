import { expect, test, type Page } from "@playwright/test";

// UI/IPC contract only: every native call is intercepted. No Board, SSH, or flasher runs.
async function mockManagement(
  page: Page,
  initialPhase = "idle",
  registered = false,
) {
  await page.addInitScript(
    ({ initialPhase, registered }) => {
      const scope = window as unknown as {
        isTauri: boolean;
        __managementCalls: { command: string; args: Record<string, unknown> }[];
        __TAURI_INTERNALS__: {
          invoke: (
            command: string,
            args?: Record<string, unknown>,
          ) => Promise<unknown>;
        };
      };
      scope.isTauri = true;
      scope.__managementCalls = [];
      const board = {
        adbSerial: "USB_TEST",
        serial: "42",
        username: "arduino",
        hostname: "test-unoq",
        os: "debian",
        osVersion: "13",
        imageVersion: "test-build",
      };
      const image = {
        version: "20260528-558",
        os: "Debian GNU/Linux 13",
        url: "https://downloads.arduino.cc/test-only.tar.zst",
        sha256: "a".repeat(64),
      };
      let phase = initialPhase;
      scope.__TAURI_INTERNALS__ = {
        invoke: async (command, args = {}) => {
          scope.__managementCalls.push({ command, args });
          if (command === "list_boards")
            return registered
              ? [{ boardId: "known-board", name: "Registered test UNO Q" }]
              : [];
          if (command === "initial_companion_package")
            return { release: "e".repeat(64), bytes: 2048 };
          if (command === "inspect_initial_companion")
            return { phase: "absent", offset: 0, started: false };
          if (command === "request_verified_board_restart")
            return { state: "reboot_scheduled" };
          if (command === "verify_board_restart")
            return {
              message:
                "Mock new kernel and pinned LAN/USB verified; no physical run.",
            };
          if (command === "renew_companion_identity")
            return {
              message: "Mock same-key renewal verified; no firmware operation.",
            };
          if (command === "prepare_initial_companion")
            return {
              phase: "identity_ready",
              message:
                "Mock preparation only: no app was started and no firmware was written.",
            };
          if (
            command === "correct_prepared_companion" ||
            command === "restore_prepared_companion"
          )
            return {
              correctionPhase:
                command === "correct_prepared_companion"
                  ? "corrected"
                  : "restored",
              message:
                "Mock source correction only: identity preserved, no app start or firmware upload.",
            };
          if (command === "app_update_info") return { version: "0.1.0" };
          if (command === "open_app_releases") return null;
          if (command === "runtime_bundle_info")
            return {
              release: "b".repeat(64),
              contract: "c".repeat(64),
              bytes: 1234,
            };
          if (command === "runtime_update_status")
            return {
              phase: "uploading",
              current: "a".repeat(64),
              offset: 8192,
              supervisor_ready: true,
            };
          if (command === "update_companion_runtime")
            return {
              phase: "committed",
              release: "b".repeat(64),
              message: "Mock runtime update verified; no hardware execution.",
            };
          if (command === "abort_companion_runtime_update")
            return {
              phase: "committed",
              release: "b".repeat(64),
              message:
                "Mock recovery verified; an already committed update was not undone.",
            };
          if (command === "load_progress" || command === "load_setup")
            return null;
          if (command === "save_progress")
            return Number(args.expectedRevision) + 1;
          if (command === "factory_restore_status")
            return {
              phase,
              bytes: 0,
              message: "Mocked native status; no hardware execution.",
            };
          if (command === "inspect_setup_usb")
            return [{ serial: "USB_TEST", state: "device" }];
          if (command === "admin_inspect_usb" || command === "admin_login")
            return board;
          if (command === "admin_action")
            return { ssids: ["Makers-net"], state: "configured" };
          if (command === "recovery_images") return [image];
          if (command === "prepare_factory_restore")
            return { planId: 1, board, image };
          if (command === "start_factory_restore") {
            phase = "flashing";
            return {
              phase,
              bytes: 0,
              message: "Mock only; no flasher was executed.",
            };
          }
          if (command === "verify_factory_boot") {
            const source =
              phase === "download_interrupted"
                ? "no_flash_started"
                : args.officialRecoveryConfirmed
                  ? "human_official_recovery"
                  : "tinkeroo_flasher";
            phase = "idle";
            return { board, source };
          }
          if (command === "disconnect_board" || command === "admin_logout")
            return null;
          throw new Error(`Unexpected mock command: ${command}`);
        },
      };
    },
    { initialPhase, registered },
  );
  await page.goto("/");
  await page
    .getByRole("button", { name: "☰ Board Setup", exact: true })
    .click();
  await page
    .getByText("Parent setup & advanced maintenance", { exact: true })
    .click();
  const management = page.getByRole("region", {
    name: "Board configuration and recovery",
  });
  if (initialPhase !== "idle") return management;
  await management
    .getByRole("button", { name: "Refresh USB Boards", exact: true })
    .click();
  await management
    .getByRole("combobox", { name: "USB Board serial", exact: true })
    .selectOption("USB_TEST");
  await management
    .getByRole("button", { name: "Inspect selected UNO Q", exact: true })
    .click();
  await management
    .getByRole("checkbox", {
      name: "I physically verified this USB Board and its displayed serial",
      exact: true,
    })
    .check();
  return management;
}

async function managementCalls(page: Page) {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __managementCalls: {
            command: string;
            args: Record<string, unknown>;
          }[];
        }
      ).__managementCalls,
  );
}

test("tracked restart distinguishes scheduling from explicit authenticated LAN and USB observation", async ({
  page,
}) => {
  const management = await mockManagement(page, "idle", true);
  await management
    .getByLabel("Linux password (this request only)")
    .fill("fixture-only-password");
  await management
    .getByRole("button", { name: "Authenticate selected Board", exact: true })
    .click();
  await management
    .getByText("Restart, pairing cleanup, and factory recovery", {
      exact: true,
    })
    .click();
  await management
    .getByLabel("Type RESTART 42", { exact: true })
    .fill("RESTART 42");
  await management
    .getByLabel("Linux password for a tracked restart", { exact: true })
    .fill("fixture-only-password");
  await management
    .getByRole("button", {
      name: "Schedule tracked paired-Board restart",
      exact: true,
    })
    .click();
  await expect(
    management.getByText(/This is not proof of a completed reboot/),
  ).toBeVisible();
  const verify = management.getByRole("button", {
    name: "Verify new kernel and authenticated LAN + USB",
    exact: true,
  });
  await expect(verify).toBeDisabled();
  await management
    .getByLabel("Board LAN address after restart", { exact: true })
    .fill("192.168.88.42");
  await management
    .getByRole("checkbox", {
      name: "Allow the app to contact this explicit local-network endpoint for restart verification",
      exact: true,
    })
    .check();
  await verify.click();
  await expect(
    management.getByText(
      "Mock new kernel and pinned LAN/USB verified; no physical run.",
      { exact: true },
    ),
  ).toBeVisible();
  const calls = await managementCalls(page);
  expect(
    calls.find((call) => call.command === "verify_board_restart")?.args,
  ).toEqual({
    serial: "USB_TEST",
    boardId: "known-board",
    lanHost: "192.168.88.42",
    lanPort: 7443,
    localConsent: true,
  });
  expect(calls.some((call) => call.command === "start_factory_restore")).toBe(
    false,
  );
});

test("initial Companion preparation has exact hardware consent and cannot claim firmware or wizard completion", async ({
  page,
}) => {
  const management = await mockManagement(page);
  await management
    .getByText("Prepare initial Companion files and identity", { exact: true })
    .click();
  await management
    .getByRole("button", { name: "Inspect initial preparation", exact: true })
    .click();
  await expect(
    management.getByText(/Initial preparation: absent/),
  ).toBeVisible();
  const prepare = management.getByRole("button", {
    name: "Prepare / resume initial Companion",
    exact: true,
  });
  await expect(prepare).toBeDisabled();
  await management
    .getByLabel("Type PREPARE 42", { exact: true })
    .fill("PREPARE 42");
  await prepare.click();
  await expect(
    management.getByText(
      "Mock preparation only: no app was started and no firmware was written.",
      { exact: true },
    ),
  ).toBeVisible();
  expect(
    (await managementCalls(page)).find(
      (call) => call.command === "prepare_initial_companion",
    )?.args,
  ).toEqual({ serial: "USB_TEST", confirmation: "PREPARE 42" });
  expect(
    (await managementCalls(page)).some((call) =>
      [
        "start_factory_restore",
        "update_companion_runtime",
        "pair_installed_companion",
      ].includes(call.command),
    ),
  ).toBe(false);
});

test("owned unstarted correction and restore require separate exact hardware confirmations without starting firmware", async ({
  page,
}) => {
  const management = await mockManagement(page);
  await management
    .getByText("Prepare initial Companion files and identity", { exact: true })
    .click();
  const correct = management.getByRole("button", {
    name: "Correct / resume owned unstarted profile (keeps identity)",
    exact: true,
  });
  const restore = management.getByRole("button", {
    name: "Restore original unstarted files (not safe for first start)",
    exact: true,
  });
  await expect(correct).toBeDisabled();
  await expect(restore).toBeDisabled();
  await management
    .getByLabel("Type CORRECT PREPARED 42", { exact: true })
    .fill("CORRECT PREPARED 41");
  await expect(correct).toBeDisabled();
  await management
    .getByLabel("Type CORRECT PREPARED 42", { exact: true })
    .fill("CORRECT PREPARED 42");
  await correct.click();
  await expect(
    management.getByText(
      "Mock source correction only: identity preserved, no app start or firmware upload.",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    management.getByLabel("Type CORRECT PREPARED 42", { exact: true }),
  ).toHaveValue("");
  await expect(restore).toBeDisabled();
  await management
    .getByLabel("Type RESTORE PREPARED 42", { exact: true })
    .fill("RESTORE PREPARED 42");
  await restore.click();
  const calls = await managementCalls(page);
  expect(
    calls.find((call) => call.command === "correct_prepared_companion")?.args,
  ).toEqual({ serial: "USB_TEST", confirmation: "CORRECT PREPARED 42" });
  expect(
    calls.find((call) => call.command === "restore_prepared_companion")?.args,
  ).toEqual({ serial: "USB_TEST", confirmation: "RESTORE PREPARED 42" });
  expect(
    calls.some((call) =>
      [
        "prepare_initial_companion",
        "update_companion_runtime",
        "start_factory_restore",
        "pair_installed_companion",
        "request_verified_board_restart",
      ].includes(call.command),
    ),
  ).toBe(false);
});

test("Mac app upgrades are parent-approved manual replacements, separate from Board updates", async ({
  page,
}) => {
  const management = await mockManagement(page);
  await management.getByText("Upgrade the Mac app", { exact: true }).click();
  await expect(management.getByText(/Installed Mac app: 0.1.0/)).toBeVisible();
  await expect(
    management.getByText(/Quit Tinkeroo before replacing/),
  ).toBeVisible();
  await management
    .getByRole("button", {
      name: "Open official Mac app releases",
      exact: true,
    })
    .click();
  const calls = await managementCalls(page);
  expect(
    calls.filter((call) => call.command === "open_app_releases"),
  ).toHaveLength(1);
  expect(
    calls.some((call) =>
      [
        "update_companion_runtime",
        "start_factory_restore",
        "probe_board",
      ].includes(call.command),
    ),
  ).toBe(false);
  await expect(
    management.getByText(/No app was downloaded or installed/),
  ).toBeVisible();
});

test("runtime updates require an inspected registered Board and distinct typed update/recovery consent", async ({
  page,
}) => {
  const management = await mockManagement(page, "idle", true);
  await management
    .getByText("Recoverable Linux runtime update", { exact: true })
    .click();
  await management
    .getByRole("button", {
      name: "Inspect runtime update status over USB",
      exact: true,
    })
    .click();
  await expect(management.getByText(/8192 bytes transferred/)).toBeVisible();
  const update = management.getByRole("button", {
    name: "Update / resume bundled Linux runtime",
    exact: true,
  });
  await expect(update).toBeDisabled();
  await management
    .getByLabel("Type UPDATE 42", { exact: true })
    .fill("UPDATE wrong-board");
  await expect(update).toBeDisabled();
  await management
    .getByLabel("Type UPDATE 42", { exact: true })
    .fill("UPDATE 42");
  await update.click();
  await expect(
    management.getByText(
      "Mock runtime update verified; no hardware execution.",
      { exact: true },
    ),
  ).toBeVisible();
  expect(
    (await managementCalls(page)).find(
      (call) => call.command === "update_companion_runtime",
    )?.args,
  ).toEqual({
    serial: "USB_TEST",
    boardId: "known-board",
    confirmation: "UPDATE 42",
  });
  await expect(
    management.getByLabel("Type UPDATE 42", { exact: true }),
  ).toHaveValue("");
  const abort = management.getByRole("button", {
    name: "Abort / recover pending runtime update",
    exact: true,
  });
  await expect(abort).toBeDisabled();
  await management
    .getByLabel("Type ABORT UPDATE 42", { exact: true })
    .fill("ABORT UPDATE 42");
  await abort.click();
  await expect(
    management.getByText(
      "Mock recovery verified; an already committed update was not undone.",
      { exact: true },
    ),
  ).toBeVisible();
  expect(
    (await managementCalls(page)).find(
      (call) => call.command === "abort_companion_runtime_update",
    )?.args,
  ).toEqual({
    serial: "USB_TEST",
    boardId: "known-board",
    confirmation: "ABORT UPDATE 42",
  });
  expect(
    (await managementCalls(page)).some(
      (call) =>
        call.command === "start_factory_restore" ||
        call.command === "probe_board",
    ),
  ).toBe(false);
});

test("Board Setup distinguishes credentials, restart, local cleanup, and destructive restore", async ({
  page,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("button", { name: "☰ Parent Settings", exact: true }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "☰ Board Setup", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Board Setup", exact: true }).first(),
  ).toBeVisible();
  await page
    .getByText("Parent setup & advanced maintenance", { exact: true })
    .click();
  const management = page.getByRole("region", {
    name: "Board configuration and recovery",
  });
  await expect(
    management.getByLabel("Linux username", { exact: true }),
  ).toHaveValue("arduino");
  await expect(
    management.getByLabel("Linux password (this request only)"),
  ).toHaveAttribute("type", "password");
  await expect(
    management.getByRole("button", { name: "Authenticate selected Board" }),
  ).toBeDisabled();
  await management
    .getByText("Restart, pairing cleanup, and factory recovery", {
      exact: true,
    })
    .click();
  await expect(
    management.getByRole("heading", {
      name: "Restart the UNO Q — keeps its data",
    }),
  ).toBeVisible();
  await expect(
    management.getByRole("heading", {
      name: "Forget Tinkeroo records — this Mac only",
    }),
  ).toBeVisible();
  await expect(
    management.getByRole("heading", {
      name: "Factory restore — erases the UNO Q Linux system and data",
    }),
  ).toBeVisible();
  await expect(
    management.getByRole("button", { name: "Schedule Board restart" }),
  ).toBeDisabled();
  await expect(
    management.getByRole("button", {
      name: "Prepare exact-Board restore plan (does not erase)",
    }),
  ).toBeDisabled();
  await expect(
    management.getByRole("button", {
      name: "Erase this UNO Q and restore the approved image",
    }),
  ).toHaveCount(0);
  await expect(
    management.getByText("Recovery: idle", { exact: false }),
  ).toBeVisible();
  // Preview mode has no real native management execution or credential persistence.
  expect(
    await page.evaluate(() =>
      Object.keys(localStorage).some((key) => /password|credential/i.test(key)),
    ),
  ).toBe(false);
});

test("management uses transient credential requests and clears them on navigation", async ({
  page,
}) => {
  const management = await mockManagement(page);
  const password = management.getByLabel("Linux password (this request only)");
  await password.fill("fixture-only-linux-password");
  await management
    .getByRole("button", { name: "Authenticate selected Board", exact: true })
    .click();
  await expect(password).toHaveValue("");
  await expect(
    management.getByText(
      "Linux authentication and physical Board identity verified.",
      { exact: false },
    ),
  ).toBeVisible();
  await password.fill("fixture-only-linux-password");
  await management
    .getByLabel("Board hostname", { exact: true })
    .fill("makers-unoq");
  await management
    .getByRole("button", {
      name: "Set hostname using the password above",
      exact: true,
    })
    .click();
  await expect(password).toHaveValue("");
  const wifiPassword = management.getByLabel(
    "Wi-Fi password (this request only)",
  );
  await management
    .getByLabel("Wi-Fi name / hidden SSID", { exact: true })
    .fill("Makers-net");
  await wifiPassword.fill("fixture-only-wifi-password");
  await management
    .getByRole("button", { name: "Join this Wi-Fi network", exact: true })
    .click();
  await expect(wifiPassword).toHaveValue("");
  const calls = await managementCalls(page);
  expect(
    calls.find((call) => call.command === "admin_login")?.args,
  ).toMatchObject({
    serial: "USB_TEST",
    username: "arduino",
    password: "fixture-only-linux-password",
    usbConfirmed: true,
  });
  expect(
    calls
      .filter((call) => call.command === "admin_action")
      .map((call) => call.args.action),
  ).toEqual([
    {
      operation: "hostname",
      hostname: "makers-unoq",
      password: "fixture-only-linux-password",
    },
    {
      operation: "wifi_join",
      ssid: "Makers-net",
      password: "fixture-only-wifi-password",
    },
  ]);
  expect(
    await page.evaluate(() =>
      JSON.stringify({
        local: { ...localStorage },
        session: { ...sessionStorage },
      }),
    ),
  ).not.toContain("fixture-only-");
  await password.fill("fixture-only-closed-maintenance");
  await page
    .getByText("Parent setup & advanced maintenance", { exact: true })
    .click();
  await page
    .getByRole("button", { name: "Add / pair a Board", exact: true })
    .click();
  await expect(password).toHaveValue("");
  await password.fill("fixture-only-not-submitted");
  await page.getByRole("button", { name: "⌂ Home Base", exact: true }).click();
  await page
    .getByRole("button", { name: "☰ Board Setup", exact: true })
    .click();
  await expect(password).toHaveValue("");
});

test("factory restore requires a prepared exact target, every acknowledgement, and typed hardware serial", async ({
  page,
}) => {
  const management = await mockManagement(page);
  await management
    .getByText("Restart, pairing cleanup, and factory recovery", {
      exact: true,
    })
    .click();
  await management
    .getByLabel("Absolute path to official arduino-flasher-cli", {
      exact: true,
    })
    .fill("/fixture-only/arduino-flasher-cli");
  await management
    .getByRole("button", {
      name: "Read supported official UNO Q image catalog",
      exact: true,
    })
    .click();
  await management
    .getByRole("combobox", {
      name: "Exact official image version",
      exact: true,
    })
    .selectOption("0");
  await management
    .getByRole("button", {
      name: "Prepare exact-Board restore plan (does not erase)",
      exact: true,
    })
    .click();
  const erase = management.getByRole("button", {
    name: "Erase this UNO Q and restore the approved image",
    exact: true,
  });
  await expect(erase).toBeDisabled();
  await management
    .getByLabel("Type ERASE 42", { exact: true })
    .fill("ERASE USB_TEST");
  await management
    .getByRole("checkbox", {
      name: "I backed up needed Board files",
      exact: false,
    })
    .check();
  await management
    .getByRole("checkbox", {
      name: "I followed the official UNO Q EDL preparation",
      exact: false,
    })
    .check();
  await management
    .getByRole("checkbox", {
      name: "Only the exact target Board is connected",
      exact: false,
    })
    .check();
  await expect(erase).toBeDisabled();
  await management
    .getByRole("checkbox", {
      name: "Loads/accessories are disconnected",
      exact: false,
    })
    .check();
  await expect(erase).toBeDisabled();
  await management
    .getByLabel("Type ERASE 42", { exact: true })
    .fill("ERASE 42");
  await expect(erase).toBeEnabled();
  expect(
    (await managementCalls(page)).some(
      (call) => call.command === "start_factory_restore",
    ),
  ).toBe(false);
  await erase.click();
  await expect(
    management.getByText("Recovery: flashing", { exact: false }),
  ).toBeVisible();
  await expect(erase).toBeDisabled();
  const invocation = (await managementCalls(page)).find(
    (call) => call.command === "start_factory_restore",
  );
  expect(invocation?.args).toEqual({
    planId: 1,
    confirmation: {
      phrase: "ERASE 42",
      backup: true,
      edl: true,
      onlyTarget: true,
      power: true,
    },
  });
});

for (const initialPhase of ["outcome_unknown", "download_interrupted"]) {
  test(`recovery evidence distinguishes ${initialPhase} from an app-observed restore`, async ({
    page,
  }) => {
    const management = await mockManagement(page, initialPhase);
    await management
      .getByText("Restart, pairing cleanup, and factory recovery", {
        exact: true,
      })
      .click();
    await management
      .getByRole("button", {
        name: "Refresh USB Boards for recovery verification",
        exact: true,
      })
      .click();
    await management
      .getByRole("combobox", { name: "USB serial to verify", exact: true })
      .selectOption("USB_TEST");
    const verify = management.getByRole("button", {
      name: "Verify Board boot and recovery evidence",
      exact: true,
    });
    if (initialPhase === "outcome_unknown") {
      await management
        .getByLabel("Type VERIFY followed by the hardware serial", {
          exact: true,
        })
        .fill("VERIFY 42");
      await expect(verify).toBeDisabled();
      await management
        .getByRole("checkbox", {
          name: "The official Arduino tool reported a successful destructive restore",
          exact: false,
        })
        .check();
      await management
        .getByLabel("Type RECOVERED followed by the hardware serial", {
          exact: true,
        })
        .fill("RECOVERED 42");
    } else {
      await management
        .getByLabel("Type VERIFY followed by the hardware serial", {
          exact: true,
        })
        .fill("VERIFY 42");
    }
    await verify.click();
    await expect(
      management.getByText(
        initialPhase === "outcome_unknown"
          ? "Official-tool restore completion was confirmed by you, not observed by this app."
          : "This app never started flashing and no factory restore is claimed.",
        { exact: false },
      ),
    ).toBeVisible();
    const call = (await managementCalls(page)).find(
      (call) => call.command === "verify_factory_boot",
    );
    expect(call?.args.officialRecoveryConfirmed).toBe(
      initialPhase === "outcome_unknown",
    );
    expect(
      (await managementCalls(page)).some(
        (call) => call.command === "start_factory_restore",
      ),
    ).toBe(false);
  });
}
