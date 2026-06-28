import { Checker } from '../../Checker.js';
import 'vite';
import '../../types.js';
import 'node:worker_threads';
import 'eslint';
import 'stylelint';
import '../../worker.js';

declare let createServeAndBuild: any;
declare class StylelintChecker extends Checker<'stylelint'> {
    constructor();
    init(): void;
}

export { StylelintChecker, createServeAndBuild };
