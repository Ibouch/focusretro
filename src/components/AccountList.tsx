import { useCallback, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import {
  AccountView,
  applyLayout,
  focusAccount,
  reorderAccount,
  setPrincipal,
  updateAccountProfile,
} from "../lib/commands";

const PRESET_COLORS = [
  "#ef4444",
  "#f97316",
  "#eab308",
  "#22c55e",
  "#06b6d4",
  "#3b82f6",
  "#8b5cf6",
  "#ec4899",
];

const AVAILABLE_ICONS = [
  "10",
  "11",
  "20",
  "21",
  "30",
  "31",
  "40",
  "41",
  "50",
  "51",
  "60",
  "61",
  "70",
  "71",
  "80",
  "81",
  "90",
  "91",
  "100",
  "101",
  "110",
  "111",
  "120",
  "121",
];

const LAYOUTS: {
  id: "maximize" | "split-h" | "split-v" | "grid-2x2" | "grid-3x2" | "grid-4x2";
  show: (n: number) => boolean;
  i18nKey: string;
}[] = [
  { id: "maximize", show: (n) => n >= 2, i18nKey: "layout.maximize" },
  { id: "split-h", show: (n) => n === 2, i18nKey: "layout.split_h" },
  { id: "split-v", show: (n) => n === 2, i18nKey: "layout.split_v" },
  { id: "grid-2x2", show: (n) => n === 4, i18nKey: "layout.grid_2x2" },
  { id: "grid-3x2", show: (n) => n === 6, i18nKey: "layout.grid_3x2" },
  { id: "grid-4x2", show: (n) => n === 8, i18nKey: "layout.grid_4x2" },
];
type Layout =
  | "maximize"
  | "split-h"
  | "split-v"
  | "grid-2x2"
  | "grid-3x2"
  | "grid-4x2";

function LayoutIcon({ type }: { type: Layout }) {
  // 14×14 SVG diagrams showing the tiling pattern
  switch (type) {
    case "maximize":
      return (
        <svg
          width="14"
          height="14"
          viewBox="0 0 14 14"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.2"
        >
          {/* Back square (top-right): only draw the parts not hidden by the front square */}
          <path d="M4 4 L4 1 L13 1 L13 10 L10 10" />
          {/* Front square (bottom-left): fully visible on top */}
          <rect x="1" y="4" width="9" height="9" rx="1" />
        </svg>
      );
    case "split-h":
      return (
        <svg
          width="14"
          height="14"
          viewBox="0 0 14 14"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.2"
        >
          <rect x="1" y="1" width="12" height="12" rx="1" />
          <line x1="7" y1="1" x2="7" y2="13" />
        </svg>
      );
    case "split-v":
      return (
        <svg
          width="14"
          height="14"
          viewBox="0 0 14 14"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.2"
        >
          <rect x="1" y="1" width="12" height="12" rx="1" />
          <line x1="1" y1="7" x2="13" y2="7" />
        </svg>
      );
    case "grid-2x2":
      return (
        <svg
          width="14"
          height="14"
          viewBox="0 0 14 14"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.2"
        >
          <rect x="1" y="1" width="12" height="12" rx="1" />
          <line x1="7" y1="1" x2="7" y2="13" />
          <line x1="1" y1="7" x2="13" y2="7" />
        </svg>
      );
    case "grid-3x2":
      return (
        <svg
          width="14"
          height="14"
          viewBox="0 0 14 14"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.2"
        >
          <rect x="1" y="1" width="12" height="12" rx="1" />
          <line x1="5" y1="1" x2="5" y2="13" />
          <line x1="9" y1="1" x2="9" y2="13" />
          <line x1="1" y1="7" x2="13" y2="7" />
        </svg>
      );
    case "grid-4x2":
      return (
        <svg
          width="14"
          height="14"
          viewBox="0 0 14 14"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.2"
        >
          <rect x="1" y="1" width="12" height="12" rx="1" />
          <line x1="4" y1="1" x2="4" y2="13" />
          <line x1="7" y1="1" x2="7" y2="13" />
          <line x1="10" y1="1" x2="10" y2="13" />
          <line x1="1" y1="7" x2="13" y2="7" />
        </svg>
      );
  }
}

interface Props {
  accounts: AccountView[];
  focusedName: string | null;
  onRefresh: () => void;
  onUpdate: (accounts: AccountView[]) => void;
  onFocused: (name: string) => void;
}

function AccountList({
  accounts,
  focusedName,
  onRefresh,
  onUpdate,
  onFocused,
}: Props) {
  const { t } = useTranslation();
  const [editingName, setEditingName] = useState<string | null>(null);
  const [dragState, setDragState] = useState<{
    sourceIdx: number;
    currentIdx: number;
  } | null>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const startYRef = useRef(0);
  const itemHeightRef = useRef(0);

  const handlePointerDown = useCallback(
    (idx: number) => (e: React.PointerEvent) => {
      if ((e.target as HTMLElement).closest("button")) return;
      e.preventDefault();
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      startYRef.current = e.clientY;

      if (listRef.current) {
        const items = listRef.current.children;
        if (items.length > 0) {
          itemHeightRef.current =
            (items[0] as HTMLElement).getBoundingClientRect().height + 4;
        }
      }

      setDragState({ sourceIdx: idx, currentIdx: idx });
    },
    [],
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!dragState) return;
      const dy = e.clientY - startYRef.current;
      const shift = Math.round(dy / itemHeightRef.current);
      const newIdx = Math.max(
        0,
        Math.min(accounts.length - 1, dragState.sourceIdx + shift),
      );
      if (newIdx !== dragState.currentIdx) {
        setDragState({ ...dragState, currentIdx: newIdx });
      }
    },
    [dragState, accounts.length],
  );

  const handlePointerUp = useCallback(async () => {
    if (!dragState) return;
    const { sourceIdx, currentIdx } = dragState;
    setDragState(null);
    if (sourceIdx !== currentIdx) {
      const name = accounts[sourceIdx].character_name;
      const reordered = [...accounts];
      const [moved] = reordered.splice(sourceIdx, 1);
      reordered.splice(currentIdx, 0, moved);
      onUpdate(reordered);
      reorderAccount(name, currentIdx).then(onUpdate);
    }
  }, [dragState, accounts, onUpdate]);

  const handleSetPrincipal = async (name: string) => {
    onUpdate(await setPrincipal(name));
  };

  const handleColorChange = async (name: string, color: string | null) => {
    const account = accounts.find((a) => a.character_name === name);
    onUpdate(
      await updateAccountProfile(name, color, account?.icon_path ?? null),
    );
  };

  const handleIconChange = async (name: string, icon: string | null) => {
    const account = accounts.find((a) => a.character_name === name);
    onUpdate(await updateAccountProfile(name, account?.color ?? null, icon));
  };

  const getDisplayOrder = () => {
    if (!dragState) return accounts.map((_, i) => i);
    const order = accounts.map((_, i) => i);
    const { sourceIdx, currentIdx } = dragState;
    order.splice(sourceIdx, 1);
    order.splice(currentIdx, 0, sourceIdx);
    return order;
  };

  const displayOrder = getDisplayOrder();
  const editingAccount = editingName
    ? accounts.find((a) => a.character_name === editingName)
    : null;

  const modalContent =
    editingAccount && !dragState ? (
      <div
        className="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50"
        onClick={(e) => {
          if (e.target === e.currentTarget) setEditingName(null);
        }}
      >
        <div
          className="w-[min(260px,90vw)] max-h-[70vh] overflow-y-auto px-3 py-2.5 bg-white dark:bg-gray-900 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex items-center justify-between mb-2">
            <p className="text-[11px] font-medium text-gray-700 dark:text-gray-300">
              {editingAccount.character_name} — {t("accounts.customize")}
            </p>
            <button
              type="button"
              onClick={() => setEditingName(null)}
              className="text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 text-lg leading-none"
              aria-label="Close"
            >
              ×
            </button>
          </div>
          <div className="space-y-2">
            <div>
              <p className="text-[10px] text-gray-500 mb-1">
                {t("accounts.color")}
              </p>
              <div className="flex gap-1.5 flex-wrap">
                <button
                  type="button"
                  onClick={() =>
                    handleColorChange(editingAccount.character_name, null)
                  }
                  className={`w-5 h-5 rounded-full border-2 bg-gray-200 dark:bg-gray-700 ${
                    (editingAccount.color ?? null) === null
                      ? "border-gray-900 dark:border-white"
                      : "border-transparent"
                  }`}
                  title={t("accounts.default_color")}
                />
                {PRESET_COLORS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    onClick={() =>
                      handleColorChange(editingAccount.character_name, c)
                    }
                    className={`w-5 h-5 rounded-full border-2 ${
                      editingAccount.color === c
                        ? "border-gray-900 dark:border-white"
                        : "border-transparent"
                    }`}
                    style={{ backgroundColor: c }}
                    title={c}
                  />
                ))}
              </div>
            </div>
            <div>
              <p className="text-[10px] text-gray-500 mb-1">
                {t("accounts.icon")}
              </p>
              <div className="flex gap-1 flex-wrap">
                <button
                  type="button"
                  onClick={() =>
                    handleIconChange(editingAccount.character_name, null)
                  }
                  className={`w-7 h-7 rounded border-2 bg-gray-100 dark:bg-gray-800 flex items-center justify-center text-[9px] text-gray-500 shrink-0 ${
                    (editingAccount.icon_path ?? null) === null
                      ? "border-gray-900 dark:border-white"
                      : "border-transparent"
                  }`}
                  title={t("accounts.no_icon")}
                >
                  ✕
                </button>
                {AVAILABLE_ICONS.map((icon) => (
                  <button
                    key={icon}
                    type="button"
                    onClick={() =>
                      handleIconChange(editingAccount.character_name, icon)
                    }
                    className={`w-7 h-7 rounded border-2 overflow-hidden flex items-center justify-center bg-gray-100 dark:bg-gray-800 shrink-0 p-0 ${
                      editingAccount.icon_path === icon
                        ? "border-gray-900 dark:border-white"
                        : "border-transparent"
                    }`}
                    title={icon}
                  >
                    <img
                      src={`/icons/${icon}.png`}
                      alt=""
                      className="w-full h-full object-cover pointer-events-none select-none"
                      draggable={false}
                    />
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>
    ) : null;

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            {t("accounts.title")}
          </h2>
          {accounts.length > 0 && (
            <span className="text-[10px] font-mono bg-gray-100 dark:bg-gray-800 text-gray-500 px-1.5 py-0.5 rounded">
              {accounts.length}
            </span>
          )}
        </div>
        <button
          onClick={onRefresh}
          className="w-6 h-6 flex items-center justify-center rounded text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
          title={t("accounts.refresh")}
        >
          <svg
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M23 4v6h-6" />
            <path d="M1 20v-6h6" />
            <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
          </svg>
        </button>
      </div>

      {/* Layout toolbar */}
      {accounts.length >= 2 && (
        <div className="flex items-center gap-1 mb-2">
          {LAYOUTS.filter((l) => l.show(accounts.length)).map(
            ({ id, i18nKey }) => (
              <button
                key={id}
                onClick={() => applyLayout(id)}
                className="w-7 h-7 flex items-center justify-center rounded text-gray-400 dark:text-gray-500 hover:text-indigo-500 dark:hover:text-indigo-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                title={t(i18nKey)}
              >
                <LayoutIcon type={id} />
              </button>
            ),
          )}
        </div>
      )}

      {/* Empty state */}
      {accounts.length === 0 ? (
        <div className="flex-1 min-h-0 flex flex-col items-center justify-center py-10">
          <img
            src="/no-accounts.png"
            alt=""
            className="w-24 h-24 object-contain opacity-90"
          />
          <p className="text-sm text-center text-gray-600 dark:text-gray-400">
            {t("accounts.empty_title")}
          </p>
          <p className="text-xs mt-1 text-center max-w-[200px] text-gray-500 dark:text-gray-500">
            {t("accounts.empty_desc")}
          </p>
        </div>
      ) : (
        <ul ref={listRef} className="space-y-1 select-none">
          {displayOrder.map((accountIdx) => {
            const account = accounts[accountIdx];
            const isDragging =
              dragState !== null && dragState.sourceIdx === accountIdx;

            return (
              <li
                key={account.window_id}
                onPointerDown={handlePointerDown(accountIdx)}
                onPointerMove={handlePointerMove}
                onPointerUp={handlePointerUp}
                className={`touch-none transition-[transform,opacity] duration-150 ease-out ${isDragging ? "opacity-60 scale-[1.02] z-10 relative" : ""}`}
              >
                <div
                  className={`group relative flex items-center h-9 bg-gray-50 dark:bg-gray-900 rounded-lg border overflow-hidden transition-colors ${
                    isDragging
                      ? "border-indigo-500 shadow-lg shadow-indigo-500/10"
                      : "border-gray-200 dark:border-gray-800 hover:border-gray-300 dark:hover:border-gray-700"
                  } cursor-grab active:cursor-grabbing`}
                >
                  {/* Colored left accent bar */}
                  <div
                    className="absolute left-0 top-0 bottom-0 w-[3px] shrink-0 dark:hidden"
                    style={{
                      backgroundColor:
                        account.character_name === focusedName
                          ? "#f97316"
                          : "#d1d5db",
                    }}
                  />
                  <div
                    className="absolute left-0 top-0 bottom-0 w-[3px] shrink-0 hidden dark:block"
                    style={{
                      backgroundColor:
                        account.character_name === focusedName
                          ? "#f97316"
                          : "#374151",
                    }}
                  />

                  {/* Drag handle */}
                  <div className="flex items-center pl-3 pr-1.5 shrink-0">
                    <svg
                      width="8"
                      height="10"
                      viewBox="0 0 8 10"
                      className="text-gray-300 dark:text-gray-600"
                      fill="currentColor"
                    >
                      <circle cx="2" cy="2" r="1" />
                      <circle cx="6" cy="2" r="1" />
                      <circle cx="2" cy="5" r="1" />
                      <circle cx="6" cy="5" r="1" />
                      <circle cx="2" cy="8" r="1" />
                      <circle cx="6" cy="8" r="1" />
                    </svg>
                  </div>

                  {/* Avatar */}
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      setEditingName(
                        editingName === account.character_name
                          ? null
                          : account.character_name,
                      );
                    }}
                    className="w-6 h-6 rounded-full shrink-0 border overflow-hidden flex items-center justify-center mr-2"
                    style={{
                      backgroundColor: account.icon_path
                        ? "transparent"
                        : (account.color ?? "#6b7280"),
                      borderColor: account.color ?? "#d1d5db",
                    }}
                    title={t("accounts.customize")}
                  >
                    {account.icon_path ? (
                      <img
                        src={`/icons/${account.icon_path}.png`}
                        alt=""
                        className="w-full h-full object-cover pointer-events-none"
                      />
                    ) : (
                      <span className="text-[9px] font-bold text-white/80 leading-none">
                        {account.character_name[0]?.toUpperCase()}
                      </span>
                    )}
                  </button>

                  {/* Name */}
                  <div className="flex-1 min-w-0">
                    <span className="text-xs font-medium truncate block text-gray-800 dark:text-gray-200">
                      {account.character_name}
                    </span>
                  </div>

                  {/* Action buttons */}
                  <div className="flex items-center gap-1 shrink-0 ml-1 pr-2">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleSetPrincipal(account.character_name);
                      }}
                      className={`w-6 h-6 flex items-center justify-center transition-colors rounded ${
                        account.is_principal
                          ? "text-amber-500 dark:text-amber-400"
                          : "text-gray-300 dark:text-gray-600 opacity-0 group-hover:opacity-100 hover:text-amber-500 dark:hover:text-amber-400"
                      }`}
                      title={
                        account.is_principal
                          ? t("accounts.principal")
                          : t("accounts.set_principal")
                      }
                    >
                      <svg
                        width="12"
                        height="12"
                        viewBox="0 0 12 12"
                        fill={account.is_principal ? "currentColor" : "none"}
                        stroke="currentColor"
                        strokeWidth="1"
                      >
                        <path d="M6 0.5l1.6 3.3 3.7.5-2.7 2.6.6 3.7L6 8.9 2.8 10.6l.6-3.7L.7 4.3l3.7-.5L6 0.5z" />
                      </svg>
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        focusAccount(account.character_name);
                        onFocused(account.character_name);
                      }}
                      className="w-6 h-6 flex items-center justify-center text-gray-400 dark:text-gray-500 opacity-0 group-hover:opacity-100 hover:text-indigo-500 dark:hover:text-indigo-400 transition-colors rounded"
                      title={t("accounts.focus_window")}
                    >
                      <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      >
                        <circle cx="12" cy="12" r="3" />
                        <path d="M12 1v4M12 19v4M1 12h4M19 12h4M4.2 4.2l2.8 2.8M17 17l2.8 2.8M4.2 19.8l2.8-2.8M17 7l2.8-2.8" />
                      </svg>
                    </button>
                  </div>
                </div>
              </li>
            );
          })}
        </ul>
      )}
      {modalContent && createPortal(modalContent, document.body)}
    </div>
  );
}

export default AccountList;
