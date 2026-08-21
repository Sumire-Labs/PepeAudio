import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { HStack, StackItem } from "@astryxdesign/core/Stack";
import {
  Typeahead,
  TypeaheadItem,
  type SearchSource,
  type SearchableItem
} from "@astryxdesign/core/Typeahead";
import { ListPlus, Search } from "lucide-react";
import { useMemo, useState } from "react";

const HISTORY_KEY = "pepeaudio-media-search-history";
const MAX_SUGGESTIONS = 5;

export interface MediaSearchSeed {
  readonly id: string;
  readonly title: string;
  readonly artist?: string | null;
}

interface MediaSuggestionData {
  readonly query: string;
  readonly description: string;
}

type MediaSuggestion = SearchableItem<MediaSuggestionData>;

interface MediaSearchBarProps {
  readonly isDisabled: boolean;
  readonly isLoading: boolean;
  readonly suggestions?: readonly MediaSearchSeed[];
  readonly onSubmit: (input: string) => Promise<void> | void;
}

export function MediaSearchBar({
  isDisabled,
  isLoading,
  suggestions = [],
  onSubmit
}: MediaSearchBarProps) {
  const [draft, setDraft] = useState("");
  const [history, setHistory] = useState<readonly string[]>(readHistory);
  const [inputRevision, setInputRevision] = useState(0);
  const searchSource = useMemo(
    () => createSearchSource(suggestions, history),
    [history, suggestions]
  );
  const trimmed = draft.trim();
  const cannotSubmit = isDisabled || isLoading || trimmed.length === 0;

  const submit = async (value: string) => {
    const query = value.trim();
    if (isDisabled || isLoading || query.length === 0) return;
    await onSubmit(query);
    const nextHistory = [query, ...history.filter((item) => item !== query)]
      .slice(0, MAX_SUGGESTIONS);
    setHistory(nextHistory);
    writeHistory(nextHistory);
    setDraft("");
    setInputRevision((revision) => revision + 1);
  };

  return (
    <HStack gap={2} width="100%" vAlign="center">
      <StackItem size="fill">
        <Typeahead<MediaSuggestion>
          key={inputRevision}
          label="曲を検索またはURLを追加"
          isLabelHidden
          searchSource={searchSource}
          value={null}
          placeholder="曲名、YouTube・Spotify・Apple MusicのURL"
          startIcon={Search}
          hasEntriesOnFocus
          hasClear
          maxMenuItems={MAX_SUGGESTIONS}
          debounceMs={0}
          width="100%"
          isDisabled={isDisabled}
          disabledMessage="利用するDiscordサーバーを選択してください。"
          emptySearchResultsText="候補がありません"
          renderItem={(item) => (
            <TypeaheadItem
              item={item}
              icon={<Icon icon={Search} />}
              {...(item.auxiliaryData?.description
                ? { description: item.auxiliaryData.description }
                : {})}
            />
          )}
          onChangeQuery={setDraft}
          onChange={(item) => {
            if (item) void submit(item.auxiliaryData?.query ?? item.label);
          }}
        />
      </StackItem>
      <IconButton
        label="キューに追加"
        tooltip="キューに追加"
        variant="primary"
        icon={<Icon icon={ListPlus} />}
        isDisabled={cannotSubmit}
        isLoading={isLoading}
        onClick={() => void submit(trimmed)}
      />
    </HStack>
  );
}

function createSearchSource(
  seeds: readonly MediaSearchSeed[],
  history: readonly string[]
): SearchSource<MediaSuggestion> {
  const known = deduplicate([
    ...seeds.map((seed) => ({
      id: `track:${seed.id}`,
      label: seed.title,
      auxiliaryData: {
        query: [seed.title, seed.artist].filter(Boolean).join(" "),
        description: seed.artist ?? "再生履歴"
      }
    })),
    ...history.map((query, index) => ({
      id: `history:${index}:${query}`,
      label: query,
      auxiliaryData: { query, description: "最近の検索" }
    }))
  ]);

  return {
    bootstrap: () => known.slice(0, MAX_SUGGESTIONS),
    search: (query) => {
      const trimmed = query.trim();
      if (trimmed.length === 0) return known.slice(0, MAX_SUGGESTIONS);
      const normalized = normalize(trimmed);
      const exactSearch: MediaSuggestion = {
        id: `query:${trimmed}`,
        label: `「${trimmed}」を検索`,
        auxiliaryData: { query: trimmed, description: "この内容でキューに追加" }
      };
      return [
        exactSearch,
        ...known.filter((item) => normalize(
          `${item.label} ${item.auxiliaryData?.description ?? ""}`
        ).includes(normalized))
      ].slice(0, MAX_SUGGESTIONS);
    }
  };
}

function deduplicate(items: readonly MediaSuggestion[]): MediaSuggestion[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    const key = normalize(item.auxiliaryData?.query ?? item.label);
    if (key.length === 0 || seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function normalize(value: string): string {
  return value.normalize("NFKC").trim().toLocaleLowerCase();
}

function readHistory(): readonly string[] {
  try {
    const value = JSON.parse(window.localStorage.getItem(HISTORY_KEY) ?? "[]") as unknown;
    if (!Array.isArray(value)) return [];
    return value.filter((item): item is string =>
      typeof item === "string"
      && item.length > 0
      && item.length <= 512
      && !/[\u0000-\u001f\u007f]/u.test(item)
    ).slice(0, MAX_SUGGESTIONS);
  } catch {
    return [];
  }
}

function writeHistory(history: readonly string[]): void {
  try {
    window.localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
  } catch {
    // Search remains usable when browser storage is unavailable.
  }
}
