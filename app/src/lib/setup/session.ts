import { z } from "zod";

export const setupSteps = [
  {
    id: "connect-usb",
    title: "Connect the Board",
    kind: "usb",
    instruction:
      "Connect the UNO Q directly with a USB-C data cable. Refresh USB devices, then explicitly select its serial; unrelated ADB devices are not selected automatically.",
  },
  {
    id: "identify",
    title: "Identify the exact UNO Q",
    kind: "human",
    instruction:
      "Compare the selected serial with the physically connected UNO Q and official Arduino App Lab. ADB visibility alone does not prove that an arbitrary device is an UNO Q.",
  },
  {
    id: "boot",
    title: "Check boot and stable power",
    kind: "human",
    instruction:
      "In official App Lab, confirm the selected UNO Q has finished booting and is not repeatedly restarting. USB device-ready means transport availability, not a verified OS/image version.",
  },
  {
    id: "name",
    title: "Choose a safe Board name",
    kind: "official",
    instruction:
      "Use official Arduino App Lab to choose a name beginning with a letter or digit and containing only letters, digits, or hyphens. This checkpoint guide hands off to official tooling; the separate existing-Board management controls do not establish first-boot compatibility.",
  },
  {
    id: "credentials",
    title: "Create or confirm Linux credentials",
    kind: "official",
    instruction:
      "Create/confirm credentials only in official Arduino setup. Tinkeroo does not ask for, store, log, or pass your Linux password here.",
  },
  {
    id: "wifi",
    title: "Join Wi-Fi or keep USB",
    kind: "official",
    instruction:
      "Configure Wi-Fi through official App Lab, or deliberately keep USB-only operation. Correct an incorrect password there and resume this checkpoint; no credentials enter this guide.",
  },
  {
    id: "network",
    title: "Enable network mode when needed",
    kind: "official",
    instruction:
      "Enable SSH explicitly in supported parent/official setup, including for the dedicated-key check over USB. LAN is also required for the complete reboot/LAN+USB proof. Do not replace unrelated SSH keys; enabling a service can expose it on local networks.",
  },
  {
    id: "pair",
    title: "Establish full USB Pairing",
    kind: "native",
    instruction:
      "Prepare the fixed baseline/identity and explicitly start it in official tooling first. Native proof must establish registration, restricted dedicated-key SSH and signed pinned-TLS authentication on the exact Board; a checkbox or legacy bearer import cannot complete this checkpoint.",
  },
  {
    id: "deploy",
    title: "Verify or update the supported Companion runtime",
    kind: "native",
    instruction:
      "The fixed compatible runtime must be current and prove authenticated disarmed trial health. Initial source preparation and first firmware startup remain a separate explicit official-tool handoff. Unsupported whole-baseline upgrades never overwrite a working deployment.",
  },
  {
    id: "rediscover",
    title: "Reboot and verify LAN plus USB",
    kind: "native",
    instruction:
      "Record intent before an approved restart, then observe a new kernel boot on the same hardware/build and the same authenticated Companion over LAN and USB. A scheduled request or lost transport is not success.",
  },
  {
    id: "hardware",
    title: "Test the Modulinos",
    kind: "native",
    instruction:
      "Run a fresh bounded self-test, move the distance input and press all three buttons. Explicitly confirm the actual light/sound observations. Missing or stale hardware cannot complete setup.",
  },
  {
    id: "crew",
    title: "Create the Crew and begin",
    kind: "native",
    instruction:
      "Persist the Crew locally after the required observed proofs. Completion is a setup record, never permission to reuse stale hardware evidence or silently reconnect/rearm.",
  },
] as const;

const serial = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[a-zA-Z0-9][a-zA-Z0-9_.:-]*$/);
export const setupProgressSchema = z.strictObject({
  schemaVersion: z.literal(1),
  selectedSerial: serial.nullable(),
  stageIndex: z.number().int().min(0).max(12),
  checkpoints: z
    .array(
      z.strictObject({
        stageId: z.string(),
        source: z.enum(["observed", "human"]),
      }),
    )
    .max(12),
});
export type SetupProgress = z.infer<typeof setupProgressSchema>;
export interface UsbDevice {
  serial: string;
  state: string;
}

export class SetupSession {
  private progress: SetupProgress;
  private devices: UsbDevice[] | null = null;
  private confirmed = false;

  constructor(saved?: unknown) {
    this.progress =
      saved === undefined
        ? {
            schemaVersion: 1,
            selectedSerial: null,
            stageIndex: 0,
            checkpoints: [],
          }
        : setupProgressSchema.parse(saved);
    const p = this.progress;
    if (
      p.checkpoints.length !== p.stageIndex ||
      (p.stageIndex > 0 && !p.selectedSerial) ||
      p.checkpoints.some(
        (checkpoint, index) =>
          checkpoint.stageId !== setupSteps[index].id ||
          checkpoint.source !==
            (index === 0 || (index >= 7 && index <= 10) ? "observed" : "human"),
      )
    ) {
      throw new Error(
        "Saved setup checkpoints are incompatible. Existing progress has not been overwritten.",
      );
    }
  }
  get step() {
    return (
      setupSteps.at(this.progress.stageIndex) ??
      ({
        id: "complete",
        title: "Setup recorded",
        kind: "complete",
        instruction:
          "Setup was recorded; reconnect and obtain fresh hardware evidence before gameplay. No automatic rearm.",
      } as const)
    );
  }
  getProgress(): SetupProgress {
    return structuredClone(this.progress);
  }
  selectBoard(value: string) {
    const selected = serial.parse(value);
    if (
      this.progress.selectedSerial &&
      selected !== this.progress.selectedSerial
    )
      throw new Error(
        "Start a new setup explicitly before changing the selected Board; checkpoints cannot transfer between Boards.",
      );
    this.progress.selectedSerial = selected;
  }
  startNew(value: string) {
    const selectedSerial = serial.parse(value);
    this.progress = {
      schemaVersion: 1,
      selectedSerial,
      stageIndex: 0,
      checkpoints: [],
    };
    this.devices = null;
    this.confirmed = false;
  }
  observeUsb(devices: UsbDevice[]) {
    this.devices = devices.map((device) => ({ ...device }));
  }
  confirmHumanStep() {
    if (this.step.kind === "native" || this.step.kind === "complete")
      throw new Error(
        "This checkpoint requires native proof; a checkbox or frontend save cannot complete it.",
      );
    if (this.step.kind === "usb")
      throw new Error(
        "USB connection needs observed evidence, not a checkbox.",
      );
    this.confirmed = true;
  }
  advance() {
    if (this.step.kind === "native" || this.step.kind === "complete")
      throw new Error(
        "This checkpoint requires native proof; a checkbox or frontend save cannot complete it.",
      );
    if (!this.devices)
      throw new Error(
        "Refresh fresh USB evidence before continuing after a restart.",
      );
    const selected = this.devices.find(
      (device) => device.serial === this.progress.selectedSerial,
    );
    if (!selected || selected.state !== "device")
      throw new Error(
        "The selected Board is not ready over USB. Check boot, authorization, power, and the data cable; unrelated devices cannot satisfy this checkpoint.",
      );
    if (this.step.kind !== "usb" && !this.confirmed)
      throw new Error(
        "Confirm this human/official-tool step explicitly first. It is not automatically verified by Tinkeroo.",
      );
    this.progress.checkpoints.push({
      stageId: this.step.id,
      source: this.step.kind === "usb" ? "observed" : "human",
    });
    this.progress.stageIndex++;
    this.confirmed = false;
  }
}
