/**
 * TypeScript client for ralph-beads-cli (Rust CLI)
 *
 * This module provides a type-safe interface to the Rust CLI binary,
 * enabling high-performance complexity detection, framework detection,
 * and iteration calculation.
 */

import { Complexity, WorkflowMode } from "./types";

/**
 * Configuration for the Rust CLI client
 */
export interface RustClientConfig {
  /** Path to the ralph-beads-cli binary (defaults to searching PATH) */
  binaryPath?: string;
  /** Shell execution function (typically from Bun or node) */
  $: any;
}

/**
 * Result from complexity detection
 */
export interface ComplexityResult {
  complexity: Complexity;
}

/**
 * Result from framework detection
 */
export interface FrameworkResult {
  framework: string;
  test_command: string;
}

/**
 * Result from iteration calculation
 */
export interface IterationResult {
  max_iterations: number;
}

/**
 * Client for interacting with ralph-beads-cli Rust binary
 */
export class RustClient {
  private binaryPath: string;
  private $: any;
  private available: boolean | null = null;

  constructor(config: RustClientConfig) {
    this.binaryPath = config.binaryPath || "ralph-beads-cli";
    this.$ = config.$;
  }

  /**
   * Check if the Rust CLI is available
   */
  async isAvailable(): Promise<boolean> {
    if (this.available !== null) {
      return this.available;
    }

    try {
      const result = await this.$`${this.binaryPath} info --format json`
        .quiet()
        .nothrow();
      this.available = result.exitCode === 0;
    } catch {
      this.available = false;
    }

    return this.available;
  }

  /**
   * Parse JSON output from the CLI
   */
  private parseJsonOutput<T>(output: any): T {
    const text = output.stdout ? output.stdout.toString() : output.text();
    return JSON.parse(text);
  }

  /**
   * Detect complexity from task description
   *
   * @param task - Task description to analyze
   * @returns Detected complexity level
   */
  async detectComplexity(task: string): Promise<Complexity> {
    const output = await this
      .$`${this.binaryPath} detect-complexity --task ${task} --format json`.quiet();
    const result = this.parseJsonOutput<ComplexityResult>(output);
    return result.complexity;
  }

  /**
   * Detect test framework from directory
   *
   * @param dir - Directory to check (defaults to current)
   * @returns Framework name and test command
   */
  async detectFramework(dir?: string): Promise<FrameworkResult> {
    const args = dir
      ? ["detect-framework", "--dir", dir, "--format", "json"]
      : ["detect-framework", "--format", "json"];

    const output = await this.$`${this.binaryPath} ${args}`.quiet();
    return this.parseJsonOutput<FrameworkResult>(output);
  }

  /**
   * Calculate max iterations for mode and complexity
   *
   * @param mode - Workflow mode (planning or building)
   * @param complexity - Complexity level
   * @returns Recommended max iterations
   */
  async calcIterations(
    mode: WorkflowMode,
    complexity: Complexity
  ): Promise<number> {
    const output = await this
      .$`${this.binaryPath} calc-iterations --mode ${mode} --complexity ${complexity} --format json`.quiet();
    const result = this.parseJsonOutput<IterationResult>(output);
    return parseInt(result.max_iterations.toString(), 10);
  }
}

/**
 * Fallback TypeScript implementations (used when Rust CLI is unavailable)
 */
export const fallback = {
  /**
   * Detect complexity using TypeScript regex (fallback)
   */
  detectComplexity(task: string): Complexity {
    const t = task.toLowerCase();

    // Critical patterns (highest priority)
    if (/auth|security|payment|migration|credential|token|encrypt|password/.test(t)) {
      return "critical";
    }

    // Trivial patterns
    if (/fix\s+typo|update\s+comment|rename|spelling|whitespace/.test(t)) {
      return "trivial";
    }

    // Simple patterns
    if (/add\s+(button|toggle|flag)|toggle|remove\s+unused|update\s+(version|dep)/.test(t)) {
      return "simple";
    }

    return "standard";
  },

  /**
   * Calculate max iterations (fallback)
   */
  calcIterations(mode: WorkflowMode, complexity: Complexity): number {
    const isPlan = mode === "planning";
    switch (complexity) {
      case "trivial":
        return isPlan ? 2 : 5;
      case "simple":
        return isPlan ? 3 : 10;
      case "critical":
        return isPlan ? 8 : 40;
      default:
        return isPlan ? 5 : 20;
    }
  },
};

/**
 * Create a RustClient with automatic fallback
 *
 * If the Rust CLI is available, uses it for better performance.
 * Otherwise, falls back to TypeScript implementations.
 */
export async function createClient(
  config: RustClientConfig
): Promise<{
  detectComplexity: (task: string) => Promise<Complexity>;
  calcIterations: (mode: WorkflowMode, complexity: Complexity) => Promise<number>;
  detectFramework: (dir?: string) => Promise<FrameworkResult>;
  isRustAvailable: boolean;
}> {
  const client = new RustClient(config);
  const isRustAvailable = await client.isAvailable();

  if (isRustAvailable) {
    return {
      detectComplexity: (task) => client.detectComplexity(task),
      calcIterations: (mode, complexity) => client.calcIterations(mode, complexity),
      detectFramework: (dir) => client.detectFramework(dir),
      isRustAvailable: true,
    };
  }

  // Fallback to TypeScript implementations
  return {
    detectComplexity: async (task) => fallback.detectComplexity(task),
    calcIterations: async (mode, complexity) => fallback.calcIterations(mode, complexity),
    detectFramework: async (_dir) => ({
      framework: "none",
      test_command: 'echo "Rust CLI not available"',
    }),
    isRustAvailable: false,
  };
}
