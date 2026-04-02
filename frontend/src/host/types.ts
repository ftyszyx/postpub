export interface HostBridge {
  openExternal(url: string): Promise<void>;
  getEnvironmentLabel(): string;
}
