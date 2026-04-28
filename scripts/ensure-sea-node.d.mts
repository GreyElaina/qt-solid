export declare function nodeArchiveInfo(version: any, platform?: NodeJS.Platform, arch?: NodeJS.Architecture): {
    directoryName: string;
    fileName: string;
    executableRelativePath: string;
};
export declare function ensureSeaNodeBinary(options?: {}): Promise<string>;
export declare function ensureSeaNodeHeadersDir(options?: {}): Promise<string>;
