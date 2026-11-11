import type { TFunction } from "i18next";
import type { ClipboardKind, ClipboardRange } from "@/types/clipboard";

/**
 * 生成空列表文案，按搜索词、范围、分类和自定义分组组合出具体提示。
 */
export function getEmptyDescription(
  t: TFunction<"clipboard">,
  keyword: string,
  range: ClipboardRange,
  category: ClipboardKind | null,
  groupId: string | null,
  groupName: string | null,
) {
  const isSearching = keyword.length > 0;
  const isFavorite = range === "favorite";
  const hasGroup = groupId !== null;
  const categoryLabel = category ? t(`empty.categories.${category}`) : "";
  const groupLabel = groupName ?? t("empty.groupFallback");

  if (isSearching) {
    return getSearchingEmptyDescription(
      t,
      keyword,
      isFavorite,
      hasGroup,
      groupLabel,
      categoryLabel,
    );
  }

  if (hasGroup) {
    if (isFavorite && category) {
      return t("empty.groupFavoriteCategory", {
        category: categoryLabel,
        group: groupLabel,
      });
    }

    if (isFavorite) {
      return t("empty.groupFavorites", { group: groupLabel });
    }

    if (category) {
      return t("empty.groupCategory", {
        category: categoryLabel,
        group: groupLabel,
      });
    }

    return t("empty.group", { group: groupLabel });
  }

  if (isFavorite && category) {
    return t("empty.favoriteCategory", { category: categoryLabel });
  }

  if (category) {
    return t("empty.category", { category: categoryLabel });
  }

  return t(isFavorite ? "empty.favorites" : "empty.history");
}

/**
 * 生成搜索空状态文案，覆盖范围 / 分类 / 分组三种过滤维度。
 */
function getSearchingEmptyDescription(
  t: TFunction<"clipboard">,
  keyword: string,
  isFavorite: boolean,
  hasGroup: boolean,
  groupLabel: string,
  categoryLabel: string,
) {
  const hasCategory = categoryLabel.length > 0;

  if (hasGroup) {
    if (isFavorite && hasCategory) {
      return t("empty.searchGroupFavoriteCategory", {
        category: categoryLabel,
        group: groupLabel,
        keyword,
      });
    }

    if (isFavorite) {
      return t("empty.searchGroupFavorites", {
        group: groupLabel,
        keyword,
      });
    }

    if (hasCategory) {
      return t("empty.searchGroupCategory", {
        category: categoryLabel,
        group: groupLabel,
        keyword,
      });
    }

    return t("empty.searchGroup", { group: groupLabel, keyword });
  }

  if (isFavorite && hasCategory) {
    return t("empty.searchFavoriteCategory", {
      category: categoryLabel,
      keyword,
    });
  }

  if (isFavorite) {
    return t("empty.searchFavorites", { keyword });
  }

  if (hasCategory) {
    return t("empty.searchCategory", { category: categoryLabel, keyword });
  }

  return t("empty.searchHistory", { keyword });
}
