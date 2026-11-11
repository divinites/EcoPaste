import { Alert, type GetRef, Input, Modal, Spin } from "antd";
import type { ChangeEvent, FC } from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getClipboardItemText, updateClipboardItemContent } from "@/commands";
import type { ClipboardItem } from "@/types/clipboard";

interface EditContentModalProps {
  /** 当前编辑目标；为 null 时关闭。 */
  item: ClipboardItem | null;
  onClose: () => void;
  /** 保存成功后通知列表按 id 回填最新记录。 */
  onSaved: (id: string) => void;
}

type TextAreaRef = GetRef<typeof Input.TextArea>;

/**
 * 文本记录内容编辑弹窗（列表层单例）。打开时从 Rust 读取完整纯文本，
 * 富文本记录保存后变为纯文本，弹窗内提示这一点。
 */
const EditContentModal: FC<EditContentModalProps> = (props) => {
  const { item, onClose, onSaved } = props;
  const { t } = useTranslation(["clipboard", "common"]);
  const [value, setValue] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const textAreaRef = useRef<TextAreaRef>(null);
  const isRichText = item?.subKind === "html" || item?.subKind === "rtf";

  useEffect(() => {
    if (!item) return;

    let cancelled = false;

    const load = async () => {
      setLoading(true);
      setValue("");

      try {
        const text = await getClipboardItemText(item.id);
        if (!cancelled) setValue(text);
      } catch {
        if (!cancelled) onClose();
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    void load();

    return () => {
      cancelled = true;
    };
  }, [item, onClose]);

  const handleChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    setValue(event.target.value);
  };

  const handleSave = async () => {
    if (!item) return;

    setSaving(true);

    try {
      await updateClipboardItemContent(item.id, value);
      onSaved(item.id);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  const handleAfterOpenChange = (open: boolean) => {
    if (!open) return;

    requestAnimationFrame(() => {
      textAreaRef.current?.focus({ cursor: "end" });
    });
  };

  return (
    <Modal
      afterOpenChange={handleAfterOpenChange}
      confirmLoading={saving}
      destroyOnHidden
      okButtonProps={{ disabled: loading || value.trim().length === 0 }}
      okText={t("common:actions.save")}
      onCancel={onClose}
      onOk={handleSave}
      open={!!item}
      title={t("clipboard:editContent.title")}
    >
      <Spin spinning={loading}>
        <div className="flex flex-col gap-2">
          {isRichText ? (
            <Alert
              showIcon
              title={t("clipboard:editContent.richTextWarning")}
              type="info"
            />
          ) : null}
          <Input.TextArea
            autoSize={{ maxRows: 14, minRows: 6 }}
            onChange={handleChange}
            placeholder={t("clipboard:editContent.placeholder")}
            ref={textAreaRef}
            value={value}
          />
        </div>
      </Spin>
    </Modal>
  );
};

export default EditContentModal;
