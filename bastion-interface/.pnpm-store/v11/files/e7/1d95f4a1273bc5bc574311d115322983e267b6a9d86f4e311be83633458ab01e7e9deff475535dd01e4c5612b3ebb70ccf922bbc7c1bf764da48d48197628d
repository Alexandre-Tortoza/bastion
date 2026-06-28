import { DiagnosticLevel, OxlintConfig } from '../../types.js';
import 'node:worker_threads';
import 'eslint';
import 'stylelint';
import 'vite';

interface ResolvedOptions {
    watchTarget: string | string[];
    logLevel?: DiagnosticLevel[];
    command: string;
}
declare function resolveOptions(root: string, config: Exclude<OxlintConfig, false>): ResolvedOptions;

export { type ResolvedOptions, resolveOptions };
