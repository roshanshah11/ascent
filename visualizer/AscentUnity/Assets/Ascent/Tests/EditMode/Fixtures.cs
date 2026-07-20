using System.IO;
using UnityEngine;

namespace Ascent.Tests.EditMode
{
    /// <summary>Loads the shared Rust-generated protocol fixtures unchanged.</summary>
    internal static class Fixtures
    {
        // Application.dataPath = <repo>/visualizer/AscentUnity/Assets
        public static string Dir =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../../../data/protocol/visualizer/v1"));

        public static byte[] ReadFrame(string relative) =>
            File.ReadAllBytes(Path.Combine(Dir, relative));

        public static string ReadText(string relative) =>
            File.ReadAllText(Path.Combine(Dir, relative));
    }
}
