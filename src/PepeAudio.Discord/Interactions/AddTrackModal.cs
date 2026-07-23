// SPDX-License-Identifier: Apache-2.0
using Discord.Interactions;

namespace PepeAudio.Discord.Interactions;

// Modal opened by the player card's "曲を追加" button.
public sealed class AddTrackModal : IModal
{
    public string Title => "曲を追加";

    [InputLabel("URL または検索ワード")]
    [ModalTextInput("query", placeholder: "https://... または 曲名で検索", maxLength: 500)]
    public string Query { get; set; } = "";
}
