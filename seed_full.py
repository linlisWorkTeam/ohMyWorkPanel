import sqlite3, uuid, time

DB = '/AI/LinlisWorkPanel/data/linlis-work-panel.sqlite3'

def new_id():
    return str(uuid.uuid4())

now = int(time.time() * 1000)
conn = sqlite3.connect(DB)
c = conn.cursor()

# Create tables (excerpt from init_db)
c.executescript('''
CREATE TABLE IF NOT EXISTS groups (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, workspace_path TEXT NOT NULL,
  owner_member_id TEXT NOT NULL, admin_member_id TEXT, created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS members (
  id TEXT PRIMARY KEY, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK(kind IN ('user','agent')), display_name TEXT NOT NULL,
  avatar_color TEXT NOT NULL, role_description TEXT NOT NULL, is_active INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS agent_profiles (
  member_id TEXT PRIMARY KEY REFERENCES members(id) ON DELETE CASCADE,
  adapter TEXT NOT NULL, executable_path TEXT, runtime_status TEXT NOT NULL DEFAULT 'unknown', updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS users (
  id TEXT PRIMARY KEY, username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL, created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
  sender_member_id TEXT NOT NULL REFERENCES members(id), parent_run_id TEXT,
  content TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS task_runs (
  id TEXT PRIMARY KEY, group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
  root_message_id TEXT NOT NULL REFERENCES messages(id), agent_member_id TEXT NOT NULL REFERENCES members(id),
  parent_run_id TEXT, depth INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL,
  output_message_id TEXT, error_message TEXT, created_at INTEGER NOT NULL,
  started_at INTEGER, completed_at INTEGER
);
''')

user_id = new_id()
password_hash = '$argon2id$v=19$m=19456,t=2,p=1$PLACEHOLDER'
c.execute('INSERT OR IGNORE INTO users(id, username, password_hash, created_at) VALUES (?,?,?,?)',
          (user_id, 'coffee_cookie', password_hash, now))

group_id = new_id()
c.execute('INSERT OR IGNORE INTO groups(id, name, workspace_path, owner_member_id, admin_member_id, created_at) VALUES (?,?,?,?,?,?)',
          (group_id, 'AI Agent 团队', '/AI/LinlisWorkPanel', user_id, None, now))

agents = [
    ('超大杯Codex', 'codex', '#2b6cb0', '项目开发主力，但不是所有时间都在线'),
    ('啥都敢干的Cursor', 'cursor', '#38a169', '项目开发关键贡献者，能做很多需要高智商才能完成的事情'),
    ('产品经理OpenClaw', 'openclaw', '#d69e2e', '负责产品设计、拉通对齐和运维'),
    ('纯牛马Codex', 'codex', '#805ad5', '超大杯Codex的备份，他不在的时候来写代码'),
]

for name, adapter, color, desc in agents:
    mid = new_id()
    c.execute("INSERT INTO members(id, group_id, kind, display_name, avatar_color, role_description, is_active, created_at) VALUES (?,?,'agent',?,?,?,1,?)",
              (mid, group_id, name, color, desc, now))
    c.execute('INSERT INTO agent_profiles(member_id, adapter, executable_path, runtime_status, updated_at) VALUES (?,?,NULL,?,?)',
              (mid, adapter, 'unknown', now))

c.execute("UPDATE groups SET admin_member_id = (SELECT id FROM members WHERE group_id=? AND kind='agent' AND display_name='超大杯Codex' LIMIT 1) WHERE id=?",
          (group_id, group_id))

conn.commit()
conn.close()
print(f'DONE group_id={group_id}')
