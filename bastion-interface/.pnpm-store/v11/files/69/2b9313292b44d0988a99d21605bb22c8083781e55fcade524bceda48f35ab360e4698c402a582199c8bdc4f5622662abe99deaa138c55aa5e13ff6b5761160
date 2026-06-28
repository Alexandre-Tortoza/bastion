import { Checker } from '../../Checker.js';
import 'vite';
import '../../types.js';
import 'node:worker_threads';
import 'eslint';
import 'stylelint';
import '../../worker.js';

declare let createServeAndBuild: any;
declare class TscChecker extends Checker<'typescript'> {
    constructor();
    init(): void;
}

export { TscChecker, createServeAndBuild };
