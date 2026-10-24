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
      "If using Wi-Fi, enable network/SSH mode in official Arduino setup. Do not replace unrelated SSH keys. USB-only Crews can explicitly skip network mode.",
  },
  {
    id: "pair",
    title: "Establish full USB Pairing",
    kind: "unsupported",
    instruction:
      "Installation-key registration, signed gameplay sessions, and physical revocation are available separately for an already installed, TBP-configured Companion. Full wizard integration, dedicated-key SSH login verification, and deployment are not integrated. Neither that registration nor development bearer-record import can mark this checkpoint complete.",
  },
  {
    id: "deploy",
    title: "Install or upgrade the Companion atomically",
    kind: "unsupported",
    instruction:
      "Automatic atomic deployment/rollback is not integrated. Use the documented official App CLI bring-up procedure; never overwrite a working deployment speculatively.",
  },
  {
    id: "rediscover",
    title: "Reboot and verify LAN plus USB",
    kind: "unsupported",
    instruction:
      "Once supported deployment is available, verify the same stable Board identity through LAN and USB after reboot. Do not treat expected transport loss alone as reboot success.",
  },
  {
    id: "hardware",
    title: "Test the Modulinos",
    kind: "unsupported",
    instruction:
      "The playable Mission supplies a fresh input/light/sound self-test. Setup must later consume its actual result, not infer success from a connected icon.",
  },
  {
    id: "crew",
    title: "Create the Crew and begin",
    kind: "unsupported",
    instruction:
      "Named shared Crew roles and resumable Treasure Guardian are available in Mission. Finishing that Mission is not proof that unsupported factory setup was completed.",
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
  stageIndex: z.number().int().min(0).max(7),
  checkpoints: z
    .array(
      z.strictObject({
        stageId: z.string(),
        source: z.enum(["observed", "human"]),
      }),
    )
    .max(7),
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
          checkpoint.source !== (index === 0 ? "observed" : "human"),
      )
    ) {
      throw new Error(
        "Saved setup checkpoints are incompatible. Existing progress has not been overwritten.",
      );
    }
  }
  get step() {
    return setupSteps[this.progress.stageIndex];
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
    if (this.step.kind === "unsupported")
      throw new Error(this.step.instruction);
    if (this.step.kind === "usb")
      throw new Error(
        "USB connection needs observed evidence, not a checkbox.",
      );
    this.confirmed = true;
  }
  advance() {
    if (this.step.kind === "unsupported")
      throw new Error(this.step.instruction);
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
