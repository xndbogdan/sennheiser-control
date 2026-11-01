// Byte pipes to a headset: Bluetooth (Web Serial over RFCOMM) or the simulated MOMENTUM 4 from the shared core.

import type { Sim } from "./core/core";

export interface Link {
  write(bytes: Uint8Array): Promise<void>;
  close(): Promise<void>;
}

export interface LinkHandlers {
  data(bytes: Uint8Array): void;
  closed(reason?: string): void;
}

/** RFCOMM service of the GAIA v3 control channel (PROTOCOL.md §1). */
export const GAIA3_SPP_UUID = "a2129ff3-081b-4c45-8afe-469d9c4842ec";

export const serialSupported = () => typeof navigator !== "undefined" && !!navigator.serial;

const isGaiaPort = (p: SerialPort) => String(p.getInfo().bluetoothServiceClassId ?? "").toLowerCase() === GAIA3_SPP_UUID;

/** A headset this browser has already been allowed to use (reconnects without the picker). */
export async function knownPort(): Promise<SerialPort | undefined> {
  return (await navigator.serial?.getPorts())?.find(isGaiaPort);
}

/** Chrome's device picker, narrowed to headsets with the GAIA control channel. */
export function pickPort(): Promise<SerialPort> {
  if (!navigator.serial) throw new Error("This browser can't open Bluetooth serial ports.");
  return navigator.serial.requestPort({
    filters: [{ bluetoothServiceClassId: GAIA3_SPP_UUID }],
    allowedBluetoothServiceClassIds: [GAIA3_SPP_UUID],
  });
}

export async function serialLink(port: SerialPort, on: LinkHandlers): Promise<Link> {
  // the baud rate is meaningless over Bluetooth, but the API requires one
  await port.open({ baudRate: 115200 });
  const reader = port.readable!.getReader();
  const writer = port.writable!.getWriter();
  let closing = false;
  (async () => {
    try {
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        if (value?.length) on.data(value);
      }
      on.closed();
    } catch (e) {
      on.closed(closing ? undefined : e instanceof Error ? e.message : String(e));
    }
  })();
  return {
    write: (bytes) => writer.write(bytes),
    async close() {
      closing = true;
      await reader.cancel().catch(() => {});
      reader.releaseLock();
      writer.releaseLock();
      await port.close().catch(() => {});
    },
  };
}

/** The simulated headset, answering like the real one (a few ms later, as over Bluetooth). */
export function simLink(sim: Sim, on: LinkHandlers): Link {
  let open = true;
  const deliver = (bytes: Uint8Array) => {
    if (bytes.length) setTimeout(() => open && on.data(bytes), 12);
  };
  // the desktop simulator drains 1 % every 20 s
  const drain = setInterval(() => deliver(sim.tick()), 20_000);
  return {
    async write(bytes) {
      if (open) deliver(sim.receive(bytes));
    },
    async close() {
      open = false;
      clearInterval(drain);
      sim.free();
      on.closed();
    },
  };
}
