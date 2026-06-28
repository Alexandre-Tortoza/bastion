function forceNoEmitOnSolutionBuilderHost(ts, host) {
  var _a;
  const parseConfigHost = {
    useCaseSensitiveFileNames: ts.sys.useCaseSensitiveFileNames,
    readDirectory: ts.sys.readDirectory,
    fileExists: ts.sys.fileExists,
    readFile: ts.sys.readFile,
    getCurrentDirectory: ts.sys.getCurrentDirectory,
    onUnRecoverableConfigFileDiagnostic: () => {
    }
  };
  const original = (_a = host.getParsedCommandLine) == null ? void 0 : _a.bind(host);
  host.getParsedCommandLine = (fileName) => {
    const parsed = original ? original(fileName) : ts.getParsedCommandLineOfConfigFile(
      fileName,
      void 0,
      parseConfigHost
    );
    if (parsed && parsed.errors.length > 0) {
      parsed.options.noEmit = true;
    }
    return parsed;
  };
  return host;
}
export {
  forceNoEmitOnSolutionBuilderHost
};
//# sourceMappingURL=tscUtils.js.map