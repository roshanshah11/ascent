using System;
using System.Linq;
using Ascent.Runtime.Presentation;
using Newtonsoft.Json.Linq;
using NUnit.Framework;

namespace Ascent.Tests.EditMode
{
    public class LayerBindingTests
    {
        [Test]
        public void EveryEngineeringLayerDeclaresTraceableInputs()
        {
            foreach (var layer in EngineeringLayerCatalog.All)
            {
                Assert.That(layer.SourceIds, Is.Not.Empty, layer.Id);
                Assert.That(layer.Units, Is.Not.Null, layer.Id);
                Assert.That(layer.EvidencePolicy, Is.Not.EqualTo(EvidencePolicy.Unspecified), layer.Id);
            }
        }

        [Test]
        public void CameraGrammarHasFiveIdsMappedToDigits()
        {
            Assert.That(CameraDirector.Ids.Count, Is.EqualTo(5));
            Assert.That(CameraDirector.ForDigit(1), Is.EqualTo(CameraDirector.Pad));
            Assert.That(CameraDirector.ForDigit(5), Is.EqualTo(CameraDirector.Inspection));
            Assert.Throws<ArgumentOutOfRangeException>(() => CameraDirector.ForDigit(6));
        }

        [Test]
        public void SwitchingCameraRaisesChangeForNewIdOnly()
        {
            var director = new CameraDirector();
            int changes = 0;
            string last = null;
            director.ActiveChanged += id => { changes++; last = id; };
            director.SwitchTo(CameraDirector.Chase);
            director.SwitchTo(CameraDirector.Chase); // no-op
            director.SwitchTo(CameraDirector.Onboard);
            Assert.That(changes, Is.EqualTo(2));
            Assert.That(last, Is.EqualTo(CameraDirector.Onboard));
            Assert.Throws<ArgumentException>(() => director.SwitchTo("orbital"));
        }

        [Test]
        public void ExportManifestCarriesEveryProvenanceField()
        {
            var request = new ExportRequest
            {
                TraceSha256 = new string('a', 64),
                MissionId = "nasa.black-brant-ix.reference",
                CameraId = CameraDirector.Chase,
                StartSeconds = 0.0,
                EndSeconds = 10.0,
                QualityPreset = "interactive",
                Width = 1920,
                Height = 1080,
                FrameRate = 60.0,
                UnityVersion = "6000.0.79f1",
                EvidenceCaveats = new[] { "reference scenario, not a historical flight" },
            };
            var json = JObject.Parse(CinematicExporter.BuildManifest(request));
            foreach (var key in new[]
            {
                "trace_sha256", "protocol_version", "mission_id", "camera_id",
                "start_seconds", "end_seconds", "quality_preset", "width", "height",
                "frame_rate", "unity_version", "evidence_caveats",
            })
            {
                Assert.That(json.ContainsKey(key), Is.True, key);
            }
            Assert.That((int)json["protocol_version"], Is.EqualTo(1));
            Assert.That(json["evidence_caveats"].Count(), Is.EqualTo(1));
            Assert.That(CinematicExporter.FrameCount(0.0, 10.0, 60.0), Is.EqualTo(601));
        }
    }
}
