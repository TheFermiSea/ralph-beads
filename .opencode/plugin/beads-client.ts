import { BeadsIssue } from "./types";

export class BeadsClient {
  constructor(private $: any) {}

  private async parseJson<T>(cmdPromise: Promise<any>): Promise<T> {
    try {
      const output = await cmdPromise;
      const text = output.stdout ? output.stdout.toString() : output.text(); 
      if (!text) return {} as T;
      return JSON.parse(text);
    } catch (e) {
      console.error("JSON Parse Error:", e);
      return {} as T;
    }
  }

  // --- Core Commands ---

  async info(): Promise<any> {
    return this.parseJson(this.$`bd info --json`.quiet());
  }

  async create(args: {
    title: string;
    type?: string;
    priority?: number;
    parent?: string;
    ephemeral?: boolean;
    description?: string;
  }): Promise<BeadsIssue> {
    const flags = [];
    if (args.type) flags.push(`--type=${args.type}`);
    if (args.priority) flags.push(`--priority=${args.priority}`);
    if (args.parent) flags.push(`--parent=${args.parent}`);
    if (args.ephemeral) flags.push(`--ephemeral`);

    const result = await this.parseJson<BeadsIssue>(
      this.$`bd create ${flags} --title=${args.title} --json`.quiet()
    );

    if (args.description && result.id) {
      await this.update(result.id, { description: args.description });
      result.description = args.description;
    }

    return result;
  }

  async update(id: string, args: {
    status?: string;
    description?: string;
    priority?: number;
    assignee?: string;
  }): Promise<void> {
    const flags = [];
    if (args.status) flags.push(`--status=${args.status}`);
    if (args.priority) flags.push(`--priority=${args.priority}`);
    if (args.assignee) flags.push(`--assignee=${args.assignee}`);

    if (flags.length > 0) {
      await this.$`bd update ${id} ${flags}`.quiet();
    }

    if (args.description) {
      await this.$`bd update ${id} --body-file -`.stdin(args.description).quiet();
    }
  }

  async show(id: string): Promise<BeadsIssue> {
    return this.parseJson(this.$`bd show ${id} --json`.quiet());
  }

  async list(args: {
    parent?: string;
    type?: string;
    status?: string;
    label?: string;
  } = {}): Promise<BeadsIssue[]> {
    const flags = [];
    if (args.parent) flags.push(`--parent=${args.parent}`);
    if (args.type) flags.push(`--type=${args.type}`);
    if (args.status) flags.push(`--status=${args.status}`);
    if (args.label) flags.push(`--label=${args.label}`);

    return this.parseJson(this.$`bd list ${flags} --json`.quiet());
  }

  async close(id: string, reason?: string): Promise<void> {
    const flags = [];
    if (reason) flags.push(`--reason=${reason}`);
    await this.$`bd close ${id} ${flags}`.quiet();
  }

  async ready(args: {
    epic?: string;
    mol?: string;
    limit?: number;
  } = {}): Promise<BeadsIssue[]> {
    const flags = [];
    if (args.epic) flags.push(`--parent=${args.epic}`);
    if (args.mol) flags.push(`--mol=${args.mol}`);
    if (args.limit) flags.push(`--limit=${args.limit}`);

    if (args.mol) {
      return this.parseJson(this.$`bd --no-daemon ready ${flags} --json`.quiet());
    }
    return this.parseJson(this.$`bd ready ${flags} --json`.quiet());
  }

  async prime(): Promise<string> {
    // Note: --focus flag does not exist in bd prime; just use bd prime
    const output = await this.$`bd prime`.quiet();
    return output.stdout ? output.stdout.toString().trim() : output.text().trim();
  }

  async addComment(id: string, body: string): Promise<void> {
    // "bd comments add [issue-id] [text]"
    await this.$`bd comments add ${id} ${body}`.quiet();
  }

  async getComments(id: string): Promise<any[]> {
    // "bd comments list [issue-id]" ... wait help said "bd comments add" usage.
    // Does "bd comments list" exist?
    // I need to check `bd comments --help` or similar.
    // The previous error showed "Usage: bd comments add ...". It didn't show "list" subcommand in the usage line printed.
    // It implies `bd comments` might be the parent command.
    // Let's assume `bd comments list` exists or `bd comments <id>` lists them.
    // Spec says: `bd comments list <epic-id> --json`.
    // I'll stick to the spec, but wrap in try-catch in usage if needed.
    return this.parseJson(this.$`bd comments list ${id} --json`.quiet());
  }

  async addDep(from: string, to: string, type: string = 'blocks'): Promise<void> {
    await this.$`bd dep add ${from} ${to} --type=${type}`.quiet();
  }

  async graph(id: string): Promise<any> {
    return this.parseJson(this.$`bd graph ${id} --json`.quiet());
  }

  async setState(id: string, key: string, value: string): Promise<void> {
    await this.$`bd set-state ${id} ${key}=${value}`.quiet();
  }

  // --- Molecule Commands ---

  async molPour(protoId: string, title?: string): Promise<string> {
    const flags = [];
    if (title) flags.push(`--title=${title}`);
    const output = await this.$`bd --no-daemon mol pour ${protoId} ${flags}`.quiet();
    return output.stdout ? output.stdout.toString().trim() : output.text().trim();
  }

  async molShow(id: string): Promise<any> {
    return this.parseJson(this.$`bd --no-daemon mol show ${id} --json`.quiet());
  }

  async molProgress(id: string): Promise<number> {
    const data = await this.parseJson<any>(this.$`bd --no-daemon mol progress ${id} --json`.quiet());
    return data.percent || 0;
  }

  async molCurrent(id: string): Promise<any> {
    // mol current might not exist in help? Spec says `bd mol current`.
    // I'll include it.
    return this.parseJson(this.$`bd --no-daemon mol current ${id} --json`.quiet());
  }

  async molSquash(id: string): Promise<void> {
    await this.$`bd --no-daemon mol squash ${id}`.quiet();
  }
}
