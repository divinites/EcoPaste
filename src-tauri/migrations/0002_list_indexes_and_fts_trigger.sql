-- 0001 的 UPDATE 触发器对整行生效：收藏、置顶、use_count、分组等元数据更新
-- 也会删除再重建一遍 FTS 行。只在被索引的列变化时重建。
DROP TRIGGER IF EXISTS clipboard_items_au;

CREATE TRIGGER clipboard_items_au AFTER UPDATE OF search_text, note ON clipboard_items BEGIN
    INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, search_text, note)
    VALUES ('delete', old.rowid, old.search_text, old.note);
    INSERT INTO clipboard_items_fts(rowid, search_text, note)
    VALUES (new.rowid, new.search_text, new.note);
END;

-- 列表恒按 is_pinned DESC 前置，再按用户选择的排序键；历史清理按 updated_at 过滤。
CREATE INDEX IF NOT EXISTS idx_clipboard_items_pinned_updated
    ON clipboard_items (is_pinned, updated_at, created_at);
CREATE INDEX IF NOT EXISTS idx_clipboard_items_pinned_created
    ON clipboard_items (is_pinned, created_at);
CREATE INDEX IF NOT EXISTS idx_clipboard_items_pinned_use_count
    ON clipboard_items (is_pinned, use_count, created_at);
CREATE INDEX IF NOT EXISTS idx_clipboard_items_group_id
    ON clipboard_items (group_id);
