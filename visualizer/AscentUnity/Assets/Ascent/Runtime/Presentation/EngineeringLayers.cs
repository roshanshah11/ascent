using System.Collections.Generic;

namespace Ascent.Runtime.Presentation
{
    /// <summary>
    /// How strongly a displayed engineering value is supported by evidence.
    /// Mirrors the Rust evidence grading; `Unspecified` is never valid on a
    /// shipped layer.
    /// </summary>
    public enum EvidencePolicy
    {
        Unspecified = 0,
        /// <summary>Read straight from a trace channel the bridge streamed.</summary>
        DirectFromTrace,
        /// <summary>Computed from trace channels by a named relationship.</summary>
        DerivedFromTrace,
        /// <summary>Reference approximation; labeled as such in the drawer.</summary>
        Approximate,
    }

    /// <summary>
    /// A declarative binding for one engineering overlay. Every layer names the
    /// trace channels / evidence field-paths it draws from (<see cref="SourceIds"/>),
    /// its display units, and its evidence policy. No presentation component may
    /// derive canonical state from Transform/Rigidbody/VFX — only from these.
    /// </summary>
    public sealed class EngineeringLayerDescriptor
    {
        public string Id { get; }
        public IReadOnlyList<string> SourceIds { get; }
        public string Units { get; }
        public EvidencePolicy EvidencePolicy { get; }
        /// <summary>Rendered only when its backing channel is present in the trace.</summary>
        public bool RequiresChannel { get; }

        public EngineeringLayerDescriptor(
            string id,
            IReadOnlyList<string> sourceIds,
            string units,
            EvidencePolicy evidencePolicy,
            bool requiresChannel = false)
        {
            Id = id;
            SourceIds = sourceIds;
            Units = units;
            EvidencePolicy = evidencePolicy;
            RequiresChannel = requiresChannel;
        }
    }

    /// <summary>The fixed catalog of engineering layers for the vertical slice.</summary>
    public static class EngineeringLayerCatalog
    {
        public const string Trajectory = "trajectory";
        public const string Velocity = "velocity";
        public const string BodyAxes = "body_axes";
        public const string Attitude = "attitude";
        public const string StageState = "stage_state";
        public const string Events = "events";
        public const string Uncertainty = "uncertainty";

        public static readonly IReadOnlyList<EngineeringLayerDescriptor> All = new[]
        {
            new EngineeringLayerDescriptor(
                Trajectory,
                new[] { "position_x_m", "position_y_m", "position_z_m" },
                "m", EvidencePolicy.DirectFromTrace),
            new EngineeringLayerDescriptor(
                Velocity,
                new[] { "velocity_x_ms", "velocity_y_ms", "velocity_z_ms" },
                "m/s", EvidencePolicy.DirectFromTrace),
            new EngineeringLayerDescriptor(
                BodyAxes,
                new[] { "velocity_x_ms", "velocity_y_ms", "velocity_z_ms" },
                "unit", EvidencePolicy.DerivedFromTrace),
            new EngineeringLayerDescriptor(
                Attitude,
                new[] { "velocity_x_ms", "velocity_y_ms", "velocity_z_ms" },
                "quaternion", EvidencePolicy.DerivedFromTrace),
            new EngineeringLayerDescriptor(
                StageState,
                new[] { "events" },
                "state", EvidencePolicy.DirectFromTrace),
            new EngineeringLayerDescriptor(
                Events,
                new[] { "events" },
                "s", EvidencePolicy.DirectFromTrace),
            new EngineeringLayerDescriptor(
                Uncertainty,
                new[] { "uncertainty" },
                "1-sigma", EvidencePolicy.Approximate,
                requiresChannel: true),
        };

        public static IEnumerable<EngineeringLayerDescriptor> Always()
        {
            foreach (var layer in All)
                if (!layer.RequiresChannel)
                    yield return layer;
        }
    }
}
