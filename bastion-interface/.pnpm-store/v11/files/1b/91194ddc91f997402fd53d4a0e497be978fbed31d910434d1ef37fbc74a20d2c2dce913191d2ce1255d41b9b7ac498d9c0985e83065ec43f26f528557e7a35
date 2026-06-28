import { createRequire } from "node:module";
import os from "node:os";
import { stripVTControlCharacters as strip } from "node:util";
import { parentPort } from "node:worker_threads";
import colors from "picocolors";
import { WS_CHECKER_ERROR_EVENT } from "./client/index.js";
import {
  createFrame,
  lineColLocToBabelLoc,
  tsLikeLocToBabelLoc
} from "./codeFrame.js";
import {
  ACTION_TYPES,
  DiagnosticLevel
} from "./types.js";
import { isMainThread } from "./utils.js";
const _require = createRequire(import.meta.url);
const defaultLogLevel = [
  DiagnosticLevel.Warning,
  DiagnosticLevel.Error,
  DiagnosticLevel.Suggestion,
  DiagnosticLevel.Message
];
function filterLogLevel(diagnostics, level = defaultLogLevel) {
  if (Array.isArray(diagnostics)) {
    return diagnostics.filter((d) => {
      if (typeof d.level !== "number") return false;
      return level.includes(d.level);
    });
  }
  if (!diagnostics.level) return null;
  return level.includes(diagnostics.level) ? diagnostics : null;
}
function diagnosticToTerminalLog(d, name) {
  const nameInLabel = name ? `(${name})` : "";
  const boldBlack = (str) => colors.bold(colors.black(str));
  const labelMap = {
    [DiagnosticLevel.Error]: boldBlack(
      colors.bgRedBright(` ERROR${nameInLabel} `)
    ),
    [DiagnosticLevel.Warning]: boldBlack(
      colors.bgYellowBright(` WARNING${nameInLabel} `)
    ),
    [DiagnosticLevel.Suggestion]: boldBlack(
      colors.bgBlueBright(` SUGGESTION${nameInLabel} `)
    ),
    [DiagnosticLevel.Message]: boldBlack(
      colors.bgCyanBright(` MESSAGE${nameInLabel} `)
    )
  };
  const levelLabel = labelMap[d.level ?? DiagnosticLevel.Error];
  const fileLabel = `${boldBlack(colors.bgCyanBright(" FILE "))} `;
  const position = d.loc ? `${colors.yellow(d.loc.start.line)}:${colors.yellow(d.loc.start.column || "")}` : "";
  return [
    `${levelLabel} ${d.message}`,
    `${fileLabel + d.id}:${position}${os.EOL}`,
    d.codeFrame + os.EOL,
    d.conclusion
  ].filter(Boolean).join(os.EOL);
}
function diagnosticToConsoleLevel(d) {
  if (!d) return "error";
  if (d.level === DiagnosticLevel.Message) return "info";
  if (d.level === DiagnosticLevel.Suggestion) return "info";
  if (d.level === DiagnosticLevel.Warning) return "warn";
  return "error";
}
function diagnosticToRuntimeError(diagnostics) {
  const diagnosticsArray = Array.isArray(diagnostics) ? diagnostics : [diagnostics];
  const results = diagnosticsArray.map((d) => {
    let loc;
    if (d.loc) {
      loc = {
        file: d.id ?? "",
        line: d.loc.start.line,
        column: typeof d.loc.start.column === "number" ? d.loc.start.column : 0
      };
    }
    return {
      message: d.message ?? "",
      stack: typeof d.stack === "string" ? d.stack : Array.isArray(d.stack) ? d.stack.join(os.EOL) : "",
      id: d.id,
      frame: d.stripedCodeFrame,
      checkerId: d.checker,
      level: d.level,
      loc
    };
  });
  return Array.isArray(diagnostics) ? results : results[0];
}
function toClientPayload(id, diagnostics) {
  return {
    event: WS_CHECKER_ERROR_EVENT,
    data: {
      checkerId: id,
      diagnostics
    }
  };
}
function wrapCheckerSummary(checkerName, rawSummary) {
  return `[${checkerName}] ${rawSummary}`;
}
function composeCheckerSummary(checkerName, errorCount, warningCount) {
  const message = `Found ${errorCount} error${errorCount > 1 ? "s" : ""} and ${warningCount} warning${warningCount > 1 ? "s" : ""}`;
  const hasError = errorCount > 0;
  const hasWarning = warningCount > 0;
  const color = hasError ? "red" : hasWarning ? "yellow" : "green";
  return colors[color](wrapCheckerSummary(checkerName, message));
}
function normalizeTsDiagnostic(d) {
  var _a, _b, _c;
  const fileName = (_a = d.file) == null ? void 0 : _a.fileName;
  const {
    flattenDiagnosticMessageText
  } = _require("typescript");
  const message = flattenDiagnosticMessageText(d.messageText, os.EOL);
  let loc;
  const pos = d.start === void 0 ? null : (_c = (_b = d.file) == null ? void 0 : _b.getLineAndCharacterOfPosition) == null ? void 0 : _c.call(_b, d.start);
  if (pos && d.file && typeof d.start === "number" && typeof d.length === "number") {
    loc = tsLikeLocToBabelLoc({
      start: pos,
      end: d.file.getLineAndCharacterOfPosition(d.start + d.length)
    });
  }
  let codeFrame;
  if (loc) {
    codeFrame = createFrame(d.file.text, loc);
  }
  return {
    message,
    conclusion: "",
    codeFrame,
    stripedCodeFrame: codeFrame && strip(codeFrame),
    id: fileName,
    checker: "TypeScript",
    loc,
    level: d.category
  };
}
function normalizeVueTscDiagnostic(d) {
  const diagnostic = normalizeTsDiagnostic(d);
  diagnostic.checker = "vue-tsc";
  return diagnostic;
}
const isNormalizedDiagnostic = (d) => {
  return Boolean(d);
};
function normalizeEslintDiagnostic(diagnostic) {
  return diagnostic.messages.map((d) => {
    let level = DiagnosticLevel.Error;
    switch (d.severity) {
      case 0:
        level = DiagnosticLevel.Error;
        return null;
      case 1:
        level = DiagnosticLevel.Warning;
        break;
      case 2:
        level = DiagnosticLevel.Error;
        break;
    }
    const loc = lineColLocToBabelLoc(d);
    const codeFrame = createFrame(diagnostic.source ?? "", loc);
    return {
      message: `${d.message} (${d.ruleId})`,
      conclusion: "",
      codeFrame,
      stripedCodeFrame: codeFrame && strip(codeFrame),
      id: diagnostic.filePath,
      checker: "ESLint",
      loc,
      level
    };
  }).filter(isNormalizedDiagnostic);
}
function normalizeStylelintDiagnostic(diagnostic) {
  return diagnostic.warnings.map((d) => {
    var _a;
    let level = DiagnosticLevel.Error;
    switch (d.severity) {
      case "warning":
        level = DiagnosticLevel.Warning;
        break;
      case "error":
        level = DiagnosticLevel.Error;
        break;
      default:
        level = DiagnosticLevel.Error;
        return null;
    }
    const loc = lineColLocToBabelLoc(d);
    const codeFrame = createFrame(((_a = diagnostic._postcssResult) == null ? void 0 : _a.css) ?? "", loc);
    return {
      message: `${d.text} (${d.rule})`,
      conclusion: "",
      codeFrame,
      stripedCodeFrame: codeFrame && strip(codeFrame),
      id: diagnostic.source,
      checker: "Stylelint",
      loc,
      level
    };
  }).filter(isNormalizedDiagnostic);
}
function ensureCall(callback) {
  setTimeout(() => {
    callback();
  });
}
function consoleLog(value, level) {
  var _a;
  if (isMainThread) {
    console[level](value);
  } else {
    (_a = parentPort) == null ? void 0 : _a.postMessage({
      type: ACTION_TYPES.console,
      level,
      payload: value
    });
  }
}
export {
  composeCheckerSummary,
  consoleLog,
  diagnosticToConsoleLevel,
  diagnosticToRuntimeError,
  diagnosticToTerminalLog,
  ensureCall,
  filterLogLevel,
  normalizeEslintDiagnostic,
  normalizeStylelintDiagnostic,
  normalizeTsDiagnostic,
  normalizeVueTscDiagnostic,
  toClientPayload,
  wrapCheckerSummary
};
//# sourceMappingURL=logger.js.map