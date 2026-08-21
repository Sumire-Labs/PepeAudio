import { AppShell } from "@astryxdesign/core/AppShell";
import { Avatar } from "@astryxdesign/core/Avatar";
import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import { Center } from "@astryxdesign/core/Center";
import { Icon } from "@astryxdesign/core/Icon";
import { Spinner } from "@astryxdesign/core/Spinner";
import { VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import { LogIn, RefreshCw } from "lucide-react";

import type { DashboardStatus } from "../app/types";

interface LoginScreenProps {
  readonly status: DashboardStatus;
  readonly message: string | null;
  readonly onLogin: () => void;
  readonly onRetry: () => void;
}

export function LoginScreen({ status, message, onLogin, onRetry }: LoginScreenProps) {
  const connecting = status === "connecting";
  const unauthenticated = status === "unauthenticated";

  return (
    <AppShell height="fill" variant="surface" contentPadding={6} mobileNav={false}>
      <Center width="100%" height="100%" minHeight={520}>
        <Card width={420} padding={8} elevation="med">
          <VStack gap={5} hAlign="center">
            <Avatar
              src="/branding/bot-icon.png"
              name="PepeAudio"
              size="xl"
              tooltip={false}
            />
            <VStack gap={2} hAlign="center">
              <Heading level={1}>PepeAudio</Heading>
              <Text color="secondary" justify="center">
                Discordの音楽を、ブラウザから操作できます。
              </Text>
            </VStack>

            {connecting ? (
              <VStack gap={2} hAlign="center">
                <Spinner size="lg" />
                <Text color="secondary">ログイン状態を確認しています…</Text>
              </VStack>
            ) : (
              <VStack gap={3} width="100%" hAlign="center">
                {message ? (
                  <Text color="secondary" justify="center">
                    {message}
                  </Text>
                ) : null}
                <Button
                  label={unauthenticated ? "Discordでログイン" : "もう一度試す"}
                  variant="primary"
                  width="100%"
                  icon={<Icon icon={unauthenticated ? LogIn : RefreshCw} />}
                  onClick={unauthenticated ? onLogin : onRetry}
                />
              </VStack>
            )}
          </VStack>
        </Card>
      </Center>
    </AppShell>
  );
}
