-- 寄せ書きテーマ
CREATE TABLE themes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 寄せ書きメッセージ(テキスト or 手書き画像)
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    theme_id INTEGER NOT NULL REFERENCES themes(id),
    kind TEXT NOT NULL CHECK (kind IN ('text', 'drawing')),
    text_content TEXT,
    image_key TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_messages_theme_id ON messages(theme_id);
