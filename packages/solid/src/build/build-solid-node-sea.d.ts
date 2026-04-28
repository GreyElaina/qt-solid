export declare function buildSolidNodeSeaPrep(options: any): Promise<{
    bundlePath: string;
    prepBlobPath: string;
    seaConfigPath: string;
}>;
export declare function buildSolidNodeSeaExecutable(options: any): Promise<{
    bundlePath: string;
    prepBlobPath: string;
    seaConfigPath: string;
    executablePath: null;
    capabilityError: string | null;
} | {
    bundlePath: string;
    prepBlobPath: string;
    seaConfigPath: string;
    executablePath: any;
    capabilityError: null;
}>;
