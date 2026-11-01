// The parts of the Web Serial API this app uses (not in TypeScript's DOM lib yet).
// Bluetooth RFCOMM ports need Chrome/Edge 117+ on desktop: https://developer.chrome.com/blog/serial-over-bluetooth

interface SerialPortInfo {
  bluetoothServiceClassId?: string | number;
}

interface SerialPortRequestOptions {
  filters?: { bluetoothServiceClassId?: string | number }[];
  allowedBluetoothServiceClassIds?: (string | number)[];
}

interface SerialPort extends EventTarget {
  readonly readable: ReadableStream<Uint8Array> | null;
  readonly writable: WritableStream<Uint8Array> | null;
  getInfo(): SerialPortInfo;
  open(options: { baudRate: number }): Promise<void>;
  close(): Promise<void>;
}

interface Serial extends EventTarget {
  getPorts(): Promise<SerialPort[]>;
  requestPort(options?: SerialPortRequestOptions): Promise<SerialPort>;
}

interface Navigator {
  readonly serial?: Serial;
}
