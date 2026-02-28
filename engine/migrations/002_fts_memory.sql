-- 002_fts_memory.sql
-- Enables Full-Text Search for project episodic memory tasks

-- Create a virtual table using FTS5 for the task steps so they are easily searchable
CREATE VIRTUAL TABLE IF NOT EXISTS task_steps_fts USING fts5(
    task_id UNINDEXED,
    step_type UNINDEXED,
    content,
    content='task_steps',
    content_rowid='id'
);

-- Triggers to keep the FTS index up to date
-- WHEN clause prevents FTS insert for rows that will fail FK constraint,
-- avoiding a crash when SQLite tries to rollback the FTS5 virtual table.
CREATE TRIGGER IF NOT EXISTS task_steps_ai AFTER INSERT ON task_steps
WHEN EXISTS (SELECT 1 FROM tasks WHERE id = new.task_id)
BEGIN
  INSERT INTO task_steps_fts(rowid, task_id, step_type, content)
  VALUES (new.id, new.task_id, new.step_type, new.content);
END;

CREATE TRIGGER IF NOT EXISTS task_steps_ad AFTER DELETE ON task_steps BEGIN
  INSERT INTO task_steps_fts(task_steps_fts, rowid, task_id, step_type, content)
  VALUES ('delete', old.id, old.task_id, old.step_type, old.content);
END;

CREATE TRIGGER IF NOT EXISTS task_steps_au AFTER UPDATE ON task_steps BEGIN
  INSERT INTO task_steps_fts(task_steps_fts, rowid, task_id, step_type, content)
  VALUES ('delete', old.id, old.task_id, old.step_type, old.content);
  INSERT INTO task_steps_fts(rowid, task_id, step_type, content)
  VALUES (new.id, new.task_id, new.step_type, new.content);
END;
