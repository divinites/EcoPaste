import type { DragEvent, FC, MouseEvent, PointerEvent, Ref } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { popupClipboardItemMenu, startDragClipboardItem } from "@/commands";
import AssetImage from "@/components/AssetImage";
import KeyHint from "@/components/KeyHint";
import type { ItemActionLabels } from "@/constants/itemActions";
import type { ClipboardAction, ClipboardItem } from "@/types/clipboard";
import type { ItemAction } from "@/types/settings";
import { cn } from "@/utils/cn";
import { isMac } from "@/utils/is";
import ClipboardQuickActions from "./ClipboardQuickActions";
import { formatCardMeta } from "./cardMeta";
import { resolveCardBandClass } from "./cardTheme";
import FilesCard from "./FilesCard";
import ImageCard from "./ImageCard";
import NoteContentSwitcher from "./NoteContentSwitcher";
import TextCard from "./TextCard";

export type ClipboardCardVariant = "compact" | "dock";

interface ClipboardCardProps {
  item: ClipboardItem;
  /**
   * `compact` 为竖向列表的紧凑卡片；`dock` 为底部停靠面板的正方形大卡片，
   * 带类型着色标题带与底部元信息。
   */
  variant?: ClipboardCardVariant;
  isSelected?: boolean;
  /**
   * 快捷键提示字符（"1"–"9" / "0"），存在时在 app 图标上叠加 KeyHint；
   * 按下修饰键（macOS ⌘ / Windows Ctrl）+ 该数字键触发快速粘贴。
   */
  hintKey?: string;
  /**
   * 快捷键触发时执行的粘贴操作，由父级列表注入。
   */
  onQuickPaste?: () => void;
  /**
   * MOD 键按下时，URL / Email 文本以链接态展示。
   */
  isLinkActive?: boolean;
  /**
   * 点击 URL / Email 文本时打开外部链接。
   */
  onOpenLink?: () => void;
  onPointerEnter?: (event: PointerEvent<HTMLDivElement>) => void;
  onPointerLeave?: () => void;
  onPointerMove?: (event: PointerEvent<HTMLDivElement>) => void;
  onMouseDown?: (event: MouseEvent<HTMLDivElement>) => void;
  onAuxClick?: (event: MouseEvent<HTMLDivElement>) => void;
  onDoubleClick?: (event: MouseEvent<HTMLDivElement>) => void;
  availableActions?: ClipboardAction[];
  quickActions?: ItemAction[];
  quickActionLabels?: ItemActionLabels;
  onQuickAction?: (action: ItemAction) => Promise<void> | void;
  showOriginalOnHover?: boolean;
  rootRef?: Ref<HTMLDivElement>;
}

/**
 * 按 `kind` 分发到具体卡片组件，统一外层 padding / 时间戳 / 来源应用图标。
 * `isSelected` 为 true 时高亮背景与边框；指针事件由列表注入用于 hover preview；
 * 右键根节点弹出 Rust 端原生菜单（避免 tauri-apps/tauri#9470 的 muda use-after-free），
 * 点击菜单项后由列表层订阅 `clipboard://menu-action` 派发到实际处理逻辑。
 */
const ClipboardCard: FC<ClipboardCardProps> = (props) => {
  const {
    item,
    variant = "compact",
    isSelected,
    hintKey,
    onQuickPaste,
    isLinkActive,
    onOpenLink,
    onPointerEnter,
    onPointerLeave,
    onPointerMove,
    onMouseDown,
    onAuxClick,
    onDoubleClick,
    availableActions,
    quickActions = [],
    quickActionLabels,
    onQuickAction,
    showOriginalOnHover = true,
    rootRef,
  } = props;
  const { kind, sourceAppId, subKind, sourceAppIconPath, sourceAppName } = item;
  const { t } = useTranslation("clipboard");
  const [hovered, setHovered] = useState(false);
  const isDock = variant === "dock";
  const typeKey = subKind ?? kind;
  const typeLabel = t(`types.${typeKey}`);
  const body = renderBody(item, isDock, isLinkActive, onOpenLink);
  const showSensitiveIndicator = item.isSensitive && item.kind === "text";
  const showStatusIndicators = item.isPinned || showSensitiveIndicator;
  const sourceAppIcon = sourceAppId ? (
    <AssetImage
      alt={sourceAppName}
      className={cn({ "size-4": !isDock, "size-5": isDock })}
      src={sourceAppIconPath}
    />
  ) : (
    <img
      alt="EcoPaste"
      className={cn("pointer-events-none", {
        "size-4": !isDock,
        "size-5": isDock,
      })}
      src={isMac ? "/logo-mac.png" : "/logo.png"}
    />
  );
  const hintedSourceAppIcon = hintKey ? (
    <KeyHint hintKey={hintKey} onKeyPress={onQuickPaste}>
      {sourceAppIcon}
    </KeyHint>
  ) : (
    sourceAppIcon
  );

  const handleDragStart = async (event: DragEvent) => {
    event.preventDefault();

    await startDragClipboardItem(item.id);
  };

  const handleContextMenu = async (event: MouseEvent) => {
    event.preventDefault();

    const actions = availableActions ?? item.availableActions ?? [];
    const { isFavorite, isPinned, note } = item;

    if (actions.length === 0) return;

    await popupClipboardItemMenu(
      item.id,
      [...actions],
      item.groupId,
      isFavorite,
      isPinned,
      Boolean(note),
    );
  };

  const handlePointerEnter = (event: PointerEvent<HTMLDivElement>) => {
    setHovered(true);
    onPointerEnter?.(event);
  };

  const handlePointerLeave = () => {
    setHovered(false);
    onPointerLeave?.();
  };

  const content = item.note ? (
    <NoteContentSwitcher
      note={item.note}
      showOriginal={showOriginalOnHover && hovered}
    >
      {body}
    </NoteContentSwitcher>
  ) : (
    body
  );

  const rootProps = {
    "aria-selected": isSelected,
    draggable: true,
    onAuxClick,
    onContextMenu: handleContextMenu,
    onDoubleClick,
    onDragStart: handleDragStart,
    onMouseDown,
    onPointerEnter: handlePointerEnter,
    onPointerLeave: handlePointerLeave,
    onPointerMove,
    ref: rootRef,
    role: "option",
    tabIndex: -1,
  } as const;

  if (isDock) {
    const bandClass = resolveCardBandClass(item);
    const meta = formatCardMeta(item, t);

    return (
      <div
        {...rootProps}
        className={cn(
          "relative flex size-full flex-col overflow-hidden rounded-4 bg-ant-container shadow-md ring-1 ring-ant-border-secondary transition-shadow duration-150 ease-out motion-reduce:transition-none",
          {
            "shadow-lg ring-2 ring-ant-primary": isSelected,
          },
        )}
      >
        <div
          className={cn(
            "flex h-13 shrink-0 items-center justify-between gap-2 px-3 text-ant-light-solid",
            bandClass,
          )}
        >
          <div className="flex min-w-0 flex-col leading-tight">
            <span className="truncate font-medium text-sm">{typeLabel}</span>
            <span className="truncate text-xs opacity-85">
              {item.displayCreatedAt ?? item.createdAt}
            </span>
          </div>

          <div className="shrink-0 rounded-2 bg-ant-container p-1">
            {hintedSourceAppIcon}
          </div>
        </div>

        <div className="min-h-0 flex-1 overflow-hidden p-3 text-sm leading-6">
          {content}
        </div>

        <div className="flex h-8 shrink-0 items-center gap-2 px-3 text-ant-tertiary text-xs">
          <div className="flex w-10 shrink-0 items-center gap-1">
            {showStatusIndicators
              ? renderStatusIcons(item.isPinned, showSensitiveIndicator)
              : null}
          </div>

          <span className="min-w-0 flex-1 truncate text-center">{meta}</span>

          <ClipboardQuickActions
            item={item}
            labels={quickActionLabels}
            onQuickAction={onQuickAction}
            placeholder={null}
            quickActions={quickActions}
            visible={hovered}
          />
        </div>
      </div>
    );
  }

  return (
    <div
      {...rootProps}
      className={cn(
        "relative flex flex-col gap-1 overflow-hidden rounded-2 border border-ant-border-secondary p-2 transition-colors duration-150 ease-out motion-reduce:transition-none",
        {
          "border-ant-primary bg-ant-blue-1": isSelected,
          "border-ant-primary bg-ant-container": item.isPinned && !isSelected,
        },
      )}
    >
      <div className="flex items-center justify-between text-ant-secondary text-xs">
        <div className="flex min-w-0 items-center gap-1 overflow-hidden">
          {hintedSourceAppIcon}

          <span className="truncate">{typeLabel}</span>
        </div>

        <ClipboardQuickActions
          item={item}
          labels={quickActionLabels}
          onQuickAction={onQuickAction}
          quickActions={quickActions}
          visible={hovered}
        />
      </div>

      {content}
      {showStatusIndicators
        ? renderStatusIndicators(item.isPinned, showSensitiveIndicator)
        : null}
    </div>
  );
};

/**
 * 渲染紧凑卡片右下角的状态水印；仅表达状态，不参与交互。
 */
function renderStatusIndicators(isPinned: boolean, isSensitive: boolean) {
  return (
    <div className="pointer-events-none absolute right-2 bottom-2 flex items-end gap-1 text-ant-quaternary">
      {renderStatusIcons(isPinned, isSensitive, "size-5")}
    </div>
  );
}

/**
 * 置顶 / 敏感状态图标；停靠卡片放在底栏左侧，紧凑卡片放在右下角水印。
 */
function renderStatusIcons(
  isPinned: boolean,
  isSensitive: boolean,
  sizeClass = "size-4",
) {
  return (
    <>
      {isPinned ? (
        <i aria-hidden="true" className={cn("i-ph:push-pin-bold", sizeClass)} />
      ) : null}
      {isSensitive ? (
        <i aria-hidden="true" className={cn("i-lucide:key-round", sizeClass)} />
      ) : null}
    </>
  );
}

const renderBody = (
  item: ClipboardItem,
  fill: boolean,
  isLinkActive?: boolean,
  onOpenLink?: () => void,
) => {
  if (item.kind === "image") return <ImageCard {...item} fill={fill} />;

  if (item.kind === "files") return <FilesCard {...item} fill={fill} />;

  return (
    <TextCard
      {...item}
      fill={fill}
      isLinkActive={isLinkActive}
      onOpenLink={onOpenLink}
    />
  );
};

export default ClipboardCard;
