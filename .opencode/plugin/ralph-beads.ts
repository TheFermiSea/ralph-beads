import type { Plugin } from "@opencode-ai/plugin";
import { tool } from "@opencode-ai/plugin";
import { SessionState, Complexity, WorkflowMode } from "./types";
import { BeadsClient } from "./beads-client";
import { getPlanningPrompt, getBuildingPrompt } from "./prompts";
import { createClient, fallback } from "./rust-client";

// --- State Management ---
const sessions = new Map<string, SessionState>();

export function getState(sessionId: string): SessionState {
  let state = sessions.get(sessionId);
  if (!state) {
    state = {
      iterationCount: 0,
      failureCount: 0,
      filesModified: [],
      commitMade: false,
      testsRan: false
    };
    sessions.set(sessionId, state);
  }
  return state;
}

export function deleteState(sessionId: string) {
  sessions.delete(sessionId);
}

// --- Plugin Definition ---
export const RalphBeads: Plugin = async ({ client, $ }) => {
  console.log("asz: RalphBeads plugin loaded!");

  const fileExists = async (path: string) => {
      const res = await $`test -f ${path}`.nothrow().quiet();
      return res.exitCode === 0;
  };

  return {
    // --- Hooks ---
    event: async ({ event }) => {
      const sessionId = (event as any).session_id || (event as any).sessionID;
      
      if (event.type === "session.created" && sessionId) {
         getState(sessionId);
      }
      
      if (event.type === "session.deleted" && sessionId) {
         deleteState(sessionId);
      }

      if (sessionId && (event.type === "message.updated" || event.type === "message.created")) {
          const msg = (event as any).properties?.message;
          if (msg && msg.role === "assistant" && msg.content) {
              const text = Array.isArray(msg.content) 
                  ? msg.content.map((c:any) => c.text).join("") 
                  : (typeof msg.content === "string" ? msg.content : JSON.stringify(msg.content));
              
              const state = getState(sessionId);
              if (text.includes("<promise>DONE</promise>")) {
                  state.promiseMade = "DONE";
              }
              if (text.includes("<promise>PLAN_READY</promise>")) {
                  state.promiseMade = "PLAN_READY";
              }
          }
      }
    },

    "tool.execute.after": async (input) => {
      const sessionId = (input as any).sessionID || (input as any).session_id;
      if (!sessionId) return;
      
      const state = getState(sessionId);
      
      if (input.tool === "edit" || input.tool === "write" || input.tool === "morph-mcp_edit_file") {
        const path = (input.args as any).filePath || (input.args as any).path;
        if (path && typeof path === "string") {
            if (!state.filesModified.includes(path)) {
                state.filesModified.push(path);
            }
        }
      }
      
      if (input.tool === "bash") {
          const cmd = (input.args as any).command as string;
          if (cmd && /git\s+commit/.test(cmd)) {
              state.commitMade = true;
          }
      }
    },

    "experimental.chat.system.transform": async (input, output) => {
        const sessionId = (input as any).sessionID || (input as any).session_id;
        if (!sessionId) return;
        
        const state = getState(sessionId);
        if (state.mode && state.mode !== 'paused' && state.mode !== 'complete') {
            const context = `
<ralph-context>
  <mode>${state.mode}</mode>
  <epic-id>${state.epicId || 'none'}</epic-id>
  <molecule-id>${state.moleculeId || 'none'}</molecule-id>
  <iteration>${state.iterationCount}</iteration>
  <worktree>${state.worktreePath || 'none'}</worktree>
</ralph-context>
`;
            output.system.push(context);
        }
    },

    "experimental.session.compacting": async (input, output) => {
        const sessionId = (input as any).sessionID || (input as any).session_id;
        if (!sessionId) return;
        
        const state = getState(sessionId);
        if (state.mode && state.mode !== 'paused' && state.mode !== 'complete') {
            const context = `
<ralph-preserved-state>
  <mode>${state.mode}</mode>
  <epic-id>${state.epicId || 'none'}</epic-id>
  <molecule-id>${state.moleculeId || 'none'}</molecule-id>
  <iteration>${state.iterationCount}</iteration>
</ralph-preserved-state>

Resume workflow with: /ralph-beads --resume ${state.moleculeId || state.epicId}
`;
            output.context.push(context);
        }
    },

    stop: async (input) => {
      const sessionId = (input as any).sessionID || (input as any).session_id;
      if (!sessionId) return;
      
      const state = getState(sessionId);
      if (!state.mode || state.mode === 'paused' || state.mode === 'complete') {
          return;
      }

      if (state.maxIterations && state.iterationCount >= state.maxIterations) {
          return;
      }

      let complete = false;
      if (state.mode === 'planning' && state.promiseMade === 'PLAN_READY') complete = true;
      if (state.mode === 'building' && state.promiseMade === 'DONE') complete = true;

      if (complete) {
          // Cleanup worktree if exists
          if (state.worktreePath) {
              const beads = new BeadsClient($);
              
              if (state.createPr && state.branchName && state.epicId) {
                  try {
                      await $`git push -u origin ${state.branchName}`.nothrow().quiet();
                      
                      const epic = await beads.show(state.epicId);
                      const title = epic.title || "Ralph-Beads Work";
                      const body = `Completed via ralph-beads molecule ${state.moleculeId}\n\nEpic: ${state.epicId}\n\nGenerated by OpenCode plugin.`;
                      
                      await $`gh pr create --title="${title}" --body="${body}" --head="${state.branchName}"`.nothrow().quiet();
                  } catch (e) {
                      console.error("PR creation failed:", e);
                  }
              }
              
              try {
                  await $`git worktree remove --force ${state.worktreePath}`.nothrow().quiet();
                  // We should prompt to user because we did something significant
                  await client.session.prompt({
                      path: { id: sessionId },
                      body: { parts: [{ type: "text", text: `Worktree ${state.worktreePath} cleaned up. PR/Push attempted.` }] }
                  });
              } catch (e) {
                  console.error("Worktree cleanup failed:", e);
              }
          }
          return; // Allow stop
      }

      if (!complete) {
          const beads = new BeadsClient($);
          
          if (state.mode === 'building') {
              if (state.moleculeId) {
                  try {
                      const ready = await beads.ready({ mol: state.moleculeId, limit: 1 });
                      if (ready.length > 0) {
                          state.iterationCount++;
                          await client.session.prompt({
                              path: { id: sessionId },
                              body: {
                                  parts: [{ 
                                      type: "text", 
                                      text: `Ralph-Loop (Iter ${state.iterationCount}): Work remains! Next task: ${ready[0].title} (${ready[0].id})\nUse bd ready --mol ${state.moleculeId} and continue.` 
                                  }]
                              }
                          });
                          return;
                      }
                  } catch (e) {
                      console.error("Stop hook error checking ready:", e);
                  }
              }
          }

          if (state.mode === 'planning') {
               state.iterationCount++;
               await client.session.prompt({
                   path: { id: sessionId },
                   body: { 
                       parts: [{ 
                           type: "text", 
                           text: `Ralph-Loop (Iter ${state.iterationCount}): Planning not complete. You must output <promise>PLAN_READY</promise> when done.` 
                       }] 
                   }
               });
               return;
          }
      }
    },

    // --- Tools ---
    tool: {
      "ralph-cancel": tool({
        description: "Gracefully cancel an active Ralph-Beads loop.",
        args: {
          epic: tool.schema.string().optional().describe("Epic ID"),
          reason: tool.schema.string().optional().describe("Cancellation reason")
        },
        async execute(args, ctx) {
           const beads = new BeadsClient($);
           const sessionId = ctx.sessionID;
           
           let epicId = args.epic;
           if (!epicId) {
               const epics = await beads.list({ type: 'epic', label: 'ralph', status: 'in_progress' });
               if (epics.length === 0) {
                   const state = getState(sessionId);
                   if (state.epicId) epicId = state.epicId;
                   else return "No active Ralph-Beads epic found.";
               } else {
                   epics.sort((a, b) => (b.updated_at || b.created_at || "").localeCompare(a.updated_at || a.created_at || ""));
                   epicId = epics[0].id;
               }
           }
           
           const epic = await beads.show(epicId);
           if (!epic.id) return `Epic ${epicId} not found.`;
           
           const reason = args.reason || "Cancelled by user";
           await beads.setState(epicId, 'mode', 'paused');
           await beads.addComment(epicId, `[CANCELLED] ${reason}. Resume with: /ralph-beads --epic ${epicId}`);
           
           deleteState(sessionId);
           
           const tasks = await beads.list({ parent: epicId });
           const complete = tasks.filter(t => t.status === 'closed').length;
           const total = tasks.length;
           
           return `Cancelled Ralph-Beads loop.\nEpic: ${epicId} - ${epic.title}\nProgress: ${complete}/${total} tasks complete.\nResume with: /ralph-beads --epic ${epicId}`;
        }
      }),

      "ralph-status": tool({
        description: "Check status of a Ralph-Beads epic.",
        args: {
          epic: tool.schema.string().optional().describe("Epic ID"),
          verbose: tool.schema.boolean().optional().describe("Show recent logs")
        },
        async execute(args, ctx) {
           const beads = new BeadsClient($);
           
           let epicId = args.epic;
           if (!epicId) {
               const epics = await beads.list({ type: 'epic', label: 'ralph' });
               if (epics.length === 0) return "No ralph epics found.";
               epics.sort((a, b) => (b.updated_at || b.created_at || "").localeCompare(a.updated_at || a.created_at || ""));
               epicId = epics[0].id;
           }
           
           const epic = await beads.show(epicId);
           if (!epic.id) return `Epic ${epicId} not found.`;

           const tasks = await beads.list({ parent: epicId });
           const complete = tasks.filter(t => t.status === 'closed').length;
           const total = tasks.length;
           const percent = total > 0 ? Math.round((complete / total) * 100) : 0;
           
           const inProgress = tasks.filter(t => t.status === 'in_progress')[0];
           const blocked = tasks.filter(t => t.status === 'blocked').length;
           
           let out = `Epic: ${epic.id} - ${epic.title}\nStatus: ${epic.status}\nProgress: ${complete}/${total} tasks complete (${percent}%)\nBlocked: ${blocked} tasks\n`;
           if (inProgress) {
               out += `Current: ${inProgress.id} - ${inProgress.title}\n`;
           }
           
           const ready = await beads.ready({ epic: epicId, limit: 5 });
           out += `\n=== Ready to Work ===\n`;
           if (ready.length === 0) out += "None\n";
           else ready.forEach(t => out += `${t.id} - ${t.title}\n`);
           
           if (args.verbose) {
               try {
                   const comments = await beads.getComments(epicId);
                   out += `\n=== Recent Iterations ===\n`;
                   comments.slice(-5).forEach(c => out += `[${c.created_at}] ${c.body?.substring(0, 100)}...\n`);
               } catch (e) {
                   out += "\n(Could not fetch comments)\n";
               }
           }
           
           return out;
        }
      }),

      "ralph-beads": tool({
        description: "Start a Ralph-Beads workflow loop (Plan or Build mode).",
        args: {
          task: tool.schema.string().describe("Task description"),
          mode: tool.schema.string().optional().describe("Execution mode: 'plan' or 'build' (default: build)"),
          epic: tool.schema.string().optional().describe("Resume existing epic ID"),
          mol: tool.schema.string().optional().describe("Resume existing molecule ID"),
          resume: tool.schema.string().optional().describe("Fast resume molecule ID (skip setup)"),
          priority: tool.schema.number().optional().describe("Epic priority (0-4)"),
          complexity: tool.schema.string().optional().describe("Override complexity: trivial|simple|standard|critical"),
          validate: tool.schema.boolean().optional().describe("Force validation"),
          skip_validate: tool.schema.boolean().optional().describe("Skip validation"),
          worktree: tool.schema.boolean().optional().describe("Use git worktree"),
          pr: tool.schema.boolean().optional().describe("Create PR on completion"),
          max_iterations: tool.schema.number().optional().describe("Max iterations"),
          dry_run: tool.schema.boolean().optional().describe("Preview only")
        },
        async execute(args, ctx) {
           const beads = new BeadsClient($);
           const sessionId = ctx.sessionID;

           try {
               await beads.info();
           } catch (e) {
               return "ERROR: Beads not initialized. Run 'bd init' first.";
           }

           // Initialize Rust client (with fallback to TypeScript if unavailable)
           const rustClient = await createClient({ $ });

           let mode: WorkflowMode = (args.mode as WorkflowMode) || 'build';
           const task = args.task;

           if (args.resume) {
               mode = 'building';
               args.mol = args.resume;
           }

           let epicId = args.epic;
           let molId = args.mol;

           if (mode === 'building') {
               if (molId) {
                   try {
                       const mol = await beads.molShow(molId);
                       epicId = mol.proto_id || mol.epic_id;
                       if (!epicId) return `ERROR: Cannot determine epic from molecule ${molId}`;
                   } catch (e) {
                       return `ERROR: Molecule ${molId} not found`;
                   }
               } else if (epicId) {
                   try {
                       molId = await beads.molPour(epicId);
                   } catch (e) {
                       return `ERROR: Failed to pour epic ${epicId} into molecule`;
                   }
               } else {
                   const priority = args.priority || 2;
                   const epic = await beads.create({
                       type: 'epic',
                       title: `Proto: ${task}`,
                       priority,
                       labels: ['ralph', 'template']
                   });
                   epicId = epic.id;

                   if (args.mode === 'plan') {
                       mode = 'planning';
                       await beads.setState(epicId, 'mode', 'planning');
                   } else {
                       await beads.setState(epicId, 'mode', 'building');
                       molId = await beads.molPour(epicId);
                       mode = 'building';
                   }
               }
           } else {
               if (!epicId) {
                   const priority = args.priority || 2;
                   const epic = await beads.create({
                       type: 'epic',
                       title: `Proto: ${task}`,
                       priority,
                       labels: ['ralph', 'template']
                   });
                   epicId = epic.id;
               }
               await beads.setState(epicId, 'mode', 'planning');
           }

           // Use Rust CLI for framework detection (with fallback)
           let framework = "";
           let testCmd = "";
           try {
               const fwResult = await rustClient.detectFramework(".");
               framework = fwResult.framework;
               testCmd = fwResult.test_command;
           } catch {
               // Fallback to simple file checks
               if (await fileExists("Cargo.toml")) {
                   framework = "rust";
                   testCmd = "cargo test";
               } else if (await fileExists("package.json")) {
                   framework = "node";
                   testCmd = "npm test";
               } else if (await fileExists("pyproject.toml") || await fileExists("setup.py")) {
                   framework = "python";
                   testCmd = "pytest";
               }
           }

           // Use Rust CLI for complexity detection (with fallback)
           let complexity: Complexity = (args.complexity as Complexity) || 'standard';
           if (!args.complexity) {
               try {
                   complexity = await rustClient.detectComplexity(task);
               } catch {
                   // Fallback to TypeScript implementation
                   complexity = fallback.detectComplexity(task);
               }
           }

           // Use Rust CLI for iteration calculation (with fallback)
           let maxIter = args.max_iterations;
           if (!maxIter) {
               try {
                   maxIter = await rustClient.calcIterations(mode, complexity);
               } catch {
                   // Fallback to TypeScript implementation
                   maxIter = fallback.calcIterations(mode, complexity);
               }
           }

           if (args.dry_run) {
               const usingRust = rustClient.isRustAvailable ? "yes" : "no (fallback)";
               return `DRY RUN:\nMode: ${mode}\nEpic: ${epicId}\nMolecule: ${molId || 'N/A'}\nComplexity: ${complexity}\nMax Iterations: ${maxIter}\nTest Framework: ${framework}\nUsing Rust CLI: ${usingRust}`;
           }

           const state = getState(sessionId);
           state.mode = mode;
           state.epicId = epicId;
           state.moleculeId = molId;
           state.complexity = complexity;
           state.maxIterations = maxIter;
           state.iterationCount = 0;
           state.failureCount = 0;
           state.promiseMade = undefined;

           // Worktree Setup
           if (args.worktree || args.pr) {
               if (mode !== 'building' || !molId) {
                   return "ERROR: Worktree requires building mode and a molecule.";
               }
               const branchName = `molecule/${molId}`;
               const worktreePath = `../worktree-${molId}`;

               const branchExists = (await $`git rev-parse --verify ${branchName}`.nothrow().quiet()).exitCode === 0;
               if (branchExists) {
                   await $`git worktree add ${worktreePath} ${branchName}`.quiet();
               } else {
                   await $`git worktree add ${worktreePath} -b ${branchName}`.quiet();
               }

               state.worktreePath = worktreePath;
               state.branchName = branchName;
               state.createPr = args.pr;

               const prompt = getBuildingPrompt(epicId, molId!, task, testCmd);
               return `*** WORKTREE SETUP ***\nCreated isolated worktree at: ${worktreePath}\n\nPLEASE RUN THIS COMMAND FIRST:\ncd ${worktreePath}\n\n` + prompt;
           }

           if (mode === 'planning') {
               return getPlanningPrompt(epicId, task);
           } else {
               return getBuildingPrompt(epicId, molId!, task, testCmd);
           }
        }
      })
    }
  };
};
