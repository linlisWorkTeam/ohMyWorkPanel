/**
 * Compatibility facade. New code should import ContextMenu/useLongPress from
 * components/ui so every overlay follows the shared theme contract.
 */
export {
  ContextMenu as ContextActionMenu,
  useLongPress,
  type ActionItem,
  type ContextMenuProps as ContextActionMenuProps,
} from "./ui/ContextMenu";
