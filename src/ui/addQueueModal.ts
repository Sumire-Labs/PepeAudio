import { LabelBuilder, ModalBuilder, TextInputBuilder, TextInputStyle } from 'discord.js';
import { buildAddQueueModalId } from './customIds.js';

/** Text-input field's own custom id within the modal (read back via interaction.fields.getTextInputValue()). */
export const ADD_QUEUE_QUERY_FIELD_ID = 'query';

/** Shown when the panel's "➕ 曲を追加" button is clicked - collects a URL/search query via a real modal, mirroring /play's own query option exactly (same maxLength). */
export function buildAddQueueModal(guildId: string): ModalBuilder {
  const textInput = new TextInputBuilder()
    .setCustomId(ADD_QUEUE_QUERY_FIELD_ID)
    .setStyle(TextInputStyle.Short)
    .setMaxLength(200)
    .setRequired(true)
    .setPlaceholder('URLまたは検索ワード');

  const label = new LabelBuilder().setLabel('URLまたは検索ワード').setTextInputComponent(textInput);

  return new ModalBuilder().setCustomId(buildAddQueueModalId(guildId)).setTitle('曲を追加').addLabelComponents(label);
}
