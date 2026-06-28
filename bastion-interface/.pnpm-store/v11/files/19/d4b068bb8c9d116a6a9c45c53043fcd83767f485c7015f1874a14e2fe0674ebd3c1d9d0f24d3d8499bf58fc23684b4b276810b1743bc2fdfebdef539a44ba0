import { FileDiagnosticManager } from '../../FileDiagnosticManager.js';
import { ResolvedOptions } from './options.js';
import { DisplayTarget } from './types.js';
import '../../logger.js';
import '@babel/code-frame';
import '../../types.js';
import 'node:worker_threads';
import 'eslint';
import 'stylelint';
import 'vite';
import 'typescript';

declare function setupDevServer(root: string, options: ResolvedOptions, manager: FileDiagnosticManager, displayTargets: Set<DisplayTarget>): Promise<void>;

export { setupDevServer };
