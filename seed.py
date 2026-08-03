import uuid, sqlite3, time
conn.executescript(open(" " /AI/LinlisWorkPanel/src-tauri/src/db.rs\).read().split(pub fn init_db)[1].split(pub fn group_from_row)[0])

def id():
    return str(uuid.uuid4())

now = int(time.time() * 1000)

conn = sqlite3.connect('/AI/LinlisWorkPanel/data/linlis-work-panel.sqlite3')
c = conn.cursor()

# Create user: 曲奇
user_id = id()
password_hash = '$placeholder'
c.execute('INSERT OR IGNORE INTO users(id, username, password_hash, created_at) VALUES (?, ?, ?, ?)',
          (user_id, 'coffee_cookie', password_hash, now))

# Create group
group_id = id()
c.execute('INSERT OR IGNORE INTO groups(id, name, workspace_path, owner_member_id, admin_member_id, created_at) VALUES (?, ?, ?, ?, ?, ?)',
          (group_id, 'AI Agent 团队', '/AI/LinlisWorkPanel', user_id, None, now))

# Agent configurations
agents = [
    {
        'display_name': '超大杯Codex',
        'adapter': 'codex',
        'avatar_color': '#2b6cb0',
        'role_description': '项目开发主力，但不是所有时间都在线',
    },
    {
        'display_name': '啥都敢干的Cursor',
        'adapter': 'cursor',
        'avatar_color': '#38a169',
        'role_description': '项目开发关键贡献者，能做很多需要高智商才能完成的事情',
    },
    {
        'display_name': '产品经理OpenClaw',
        'adapter': 'openclaw',
        'avatar_color': '#d69e2e',
        'role_description': '负责产品设计、拉通对齐和运维',
    },
    {
        'display_name': '纯牛马Codex',
        'adapter': 'codex',
        'avatar_color': '#805ad5',
        'role_description': '超大杯Codex的备份，他不在的时候来写代码',
    },
]

for a in agents:
    member_id = id()
    c.execute(
        'INSERT INTO members(id, group_id, kind, display_name, avatar_color, role_description, is_active, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)',
        (member_id, group_id, 'agent', a['display_name'], a['avatar_color'], a['role_description'], 1, now)
    )
    c.execute(
        'INSERT INTO agent_profiles(member_id, adapter, executable_path, runtime_status, updated_at) VALUES (?, ?, ?, ?, ?)',
        (member_id, a['adapter'], None, 'unknown', now)
    )

# Update group admin to first agent (超大杯Codex)
c.execute("UPDATE groups SET admin_member_id = (SELECT id FROM members WHERE group_id = ? AND kind='agent' AND display_name='超大杯Codex' LIMIT 1) WHERE id = ?",
          (group_id, group_id))

conn.commit()
conn.close()
print('Seed completed!')
print(f'Group ID: {group_id}')
print(f'User ID: {user_id}')
print('Agents created: 超大杯Codex, 啥都敢干的Cursor, 产品经理OpenClaw, 纯牛马Codex')
