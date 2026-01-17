export type WorkflowMode = 'planning' | 'building' | 'paused' | 'complete';

export type Complexity = 'trivial' | 'simple' | 'standard' | 'critical';

export interface SessionState {
  // IDs for tracking context
  epicId?: string;
  moleculeId?: string;
  
  // Workflow state
  mode?: WorkflowMode;
  complexity?: Complexity;
  currentTask?: string;
  
  // Loop tracking
  iterationCount: number;
  failureCount: number;
  maxIterations?: number;
  promiseMade?: string; // 'PLAN_READY' | 'DONE'
  
  // Worktree
  worktreePath?: string;
  branchName?: string;
  createPr?: boolean;
  
  // Action tracking
  filesModified: string[];
  commitMade: boolean;
  testsRan: boolean;
}

// Interface for parsed 'bd' CLI output
export interface BeadsIssue {
  id: string;
  title: string;
  description?: string;
  status: string;
  priority?: number;
  issue_type: string;
  labels?: string[];
  created_at?: string;
  updated_at?: string;
  closed_at?: string;
  assignee?: string;
  
  // Optional fields that might be present in JSON output
  acceptance_criteria?: string;
  design?: string;
  notes?: string;
  dependencies?: string[]; // IDs
  dependents?: string[];   // IDs
  metadata?: Record<string, any>;
}
