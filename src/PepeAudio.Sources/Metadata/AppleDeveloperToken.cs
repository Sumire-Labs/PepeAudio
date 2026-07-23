// SPDX-License-Identifier: Apache-2.0
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace PepeAudio.Sources.Metadata;

// Builds an ES256 Apple Music developer token (JWT) from a .p8 private key.
public static class AppleDeveloperToken
{
    public static string Create(string teamId, string keyId, string p8Pem, DateTimeOffset now, TimeSpan lifetime)
    {
        var header = Segment(new Dictionary<string, string> { ["alg"] = "ES256", ["kid"] = keyId, ["typ"] = "JWT" });
        var payload = Segment(new Dictionary<string, object>
        {
            ["iss"] = teamId,
            ["iat"] = now.ToUnixTimeSeconds(),
            ["exp"] = now.Add(lifetime).ToUnixTimeSeconds(),
        });

        var signingInput = $"{header}.{payload}";
        using var ecdsa = ECDsa.Create();
        ecdsa.ImportFromPem(p8Pem);
        var signature = ecdsa.SignData(Encoding.ASCII.GetBytes(signingInput), HashAlgorithmName.SHA256,
            DSASignatureFormat.IeeeP1363FixedFieldConcatenation);

        return $"{signingInput}.{Base64Url(signature)}";
    }

    private static string Segment<TValue>(Dictionary<string, TValue> map)
        => Base64Url(Encoding.UTF8.GetBytes(JsonSerializer.Serialize(map)));

    private static string Base64Url(byte[] bytes)
        => Convert.ToBase64String(bytes).TrimEnd('=').Replace('+', '-').Replace('/', '_');
}
