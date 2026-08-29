-- テーマを「1枚の共有キャンバス」として持たせるための拡張
ALTER TABLE themes ADD COLUMN canvas_key TEXT;
ALTER TABLE themes ADD COLUMN contribution_count INTEGER NOT NULL DEFAULT 0;
