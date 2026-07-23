-- SPDX-License-Identifier: Apache-2.0
-- Aura 360° enhancer toggle (independent of the HRIR aura_enabled toggle), default off.
ALTER TABLE guild_settings ADD COLUMN IF NOT EXISTS aura360_enabled BOOLEAN NOT NULL DEFAULT false;
