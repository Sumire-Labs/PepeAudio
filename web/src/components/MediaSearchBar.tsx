import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, StackItem } from "@astryxdesign/core/Stack";
import { TextInput } from "@astryxdesign/core/TextInput";
import { ListPlus, Search } from "lucide-react";
import { useState } from "react";

interface MediaSearchBarProps {
  readonly isDisabled: boolean;
  readonly isLoading: boolean;
  readonly onSubmit: (input: string) => Promise<void> | void;
}

export function MediaSearchBar({
  isDisabled,
  isLoading,
  onSubmit
}: MediaSearchBarProps) {
  const [input, setInput] = useState("");
  const trimmed = input.trim();
  const cannotSubmit = isDisabled || isLoading || trimmed.length === 0;
  const submit = async () => {
    if (cannotSubmit) return;
    await onSubmit(trimmed);
    setInput("");
  };

  return (
    <HStack gap={2} width="100%" vAlign="center">
      <StackItem size="fill">
        <TextInput
          label="曲を検索またはURLを追加"
          isLabelHidden
          value={input}
          placeholder="曲名、YouTube・Spotify・Apple MusicのURL"
          startIcon={<Icon icon={Search} />}
          hasClear
          width="100%"
          isDisabled={isDisabled}
          disabledMessage="利用するDiscordサーバーを選択してください。"
          onChange={setInput}
          onEnter={() => void submit()}
        />
      </StackItem>
      <Button
        label="キューに追加"
        variant="primary"
        icon={<Icon icon={ListPlus} />}
        isDisabled={cannotSubmit}
        isLoading={isLoading}
        onClick={() => void submit()}
      />
    </HStack>
  );
}
