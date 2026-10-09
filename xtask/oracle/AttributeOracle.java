package crisp;

import com.google.gson.JsonPrimitive;
import com.mojang.serialization.JsonOps;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.CloseableResourceManager;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.ARGB;
import net.minecraft.util.EasingType;
import net.minecraft.util.TriState;
import net.minecraft.util.random.Weighted;
import net.minecraft.world.attribute.AmbientParticle;
import net.minecraft.world.attribute.AttributeTypes;
import net.minecraft.world.attribute.BedRule;
import net.minecraft.world.attribute.EnvironmentAttribute;
import net.minecraft.world.attribute.EnvironmentAttributeMap;
import net.minecraft.world.attribute.EnvironmentAttributeSystem;
import net.minecraft.world.attribute.EnvironmentAttributes;
import net.minecraft.world.attribute.WeatherAttributes;
import net.minecraft.world.attribute.modifier.AttributeModifier;
import net.minecraft.world.attribute.modifier.BooleanModifier;
import net.minecraft.world.attribute.modifier.ColorModifier;
import net.minecraft.world.attribute.modifier.FloatModifier;
import net.minecraft.world.attribute.modifier.FloatWithAlpha;
import net.minecraft.world.attribute.modifier.IntegerModifier;
import net.minecraft.world.attribute.modifier.MobSpawnSettingsModifier;
import net.minecraft.world.clock.ClockInstance;
import net.minecraft.world.clock.ClockManager;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.MobCategory;
import net.minecraft.world.entity.schedule.Activity;
import net.minecraft.world.level.MoonPhase;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.BiomeResolver;
import net.minecraft.world.level.biome.MobSpawnSettings;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.dimension.BuiltinDimensionTypes;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.timeline.Timeline;
import org.joml.Vector3f;
import org.joml.Vector3fc;
import org.joml.Vector4f;
import org.joml.Vector4fc;

public final class AttributeOracle {
    static final long SEED = 12345L;
    static RegistryAccess.Frozen registries;

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        loadRegistries();
        attributes();
        modifiers();
        easing();
        systems();
        positions();
    }

    static void loadRegistries() {
        PackRepository packs = ServerPacksSource.createVanillaTrustedRepository();
        packs.reload();
        packs.setSelected(List.of("vanilla"));
        CloseableResourceManager resources = new MultiPackResourceManager(PackType.SERVER_DATA, packs.openAllSelected());
        LayeredRegistryAccess<RegistryLayer> layers = RegistryLayer.createRegistryAccess();
        List<Registry.PendingTags<?>> staticTags = TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC));
        RegistryAccess.Frozen context = layers.getAccessForLoading(RegistryLayer.WORLD);
        List<HolderLookup.RegistryLookup<?>> lookups = TagLoader.buildUpdatedLookups(context, staticTags);
        registries = RegistryDataLoader.load(resources, lookups, RegistryDataLoader.WORLD_REGISTRIES, Runnable::run).join();
        staticTags.forEach(Registry.PendingTags::apply);
    }

    static String f(float v) {
        return "f:" + Integer.toHexString(Float.floatToRawIntBits(v));
    }

    static String d(double v) {
        return Long.toHexString(Double.doubleToRawLongBits(v));
    }

    static String fmt(Object v) {
        if (v instanceof Boolean b) return b.toString();
        if (v instanceof Float x) return f(x);
        if (v instanceof Integer i) return "i:" + i;
        if (v instanceof Vector4fc c) return "argb:" + f(c.x()) + "," + f(c.y()) + "," + f(c.z()) + "," + f(c.w());
        if (v instanceof Vector3fc c) return "rgb:" + f(c.x()) + "," + f(c.y()) + "," + f(c.z());
        if (v instanceof MoonPhase p) return p.getSerializedName();
        if (v instanceof TriState t) return t.getSerializedName();
        if (v instanceof Activity a) return BuiltInRegistries.ACTIVITY.getKey(a).toString();
        if (v instanceof BedRule r) {
            return "bed:" + r.canSleep().getSerializedName() + "," + r.canSetSpawn().getSerializedName() + "," + r.destroyOnUse() + "," + r.destroyOnLeave()
                + "," + r.errorMessage().isPresent();
        }
        if (v instanceof MobSpawnSettings s) {
            StringBuilder out = new StringBuilder("mobs:");
            for (MobCategory category : MobCategory.values()) {
                var list = s.getMobsInCategory(category);
                if (list != null) {
                    out.append(category.getSerializedName()).append('=');
                    for (Weighted<MobSpawnSettings.SpawnerData> w : list.unwrap()) {
                        out.append(EntityType.getKey(w.value().type())).append('*').append(w.weight()).append(',');
                    }
                    out.append(';');
                }
            }
            Map<String, String> costs = new TreeMap<>();
            s.allSpawnCosts().forEach((type, cost) -> costs.put(EntityType.getKey(type).toString(), d(cost.energyBudget()) + ":" + d(cost.charge())));
            costs.forEach((type, cost) -> out.append("cost=").append(type).append(':').append(cost).append(';'));
            return out.toString();
        }
        if (v instanceof List<?> list) {
            StringBuilder out = new StringBuilder("particles:");
            for (Object p : list) {
                out.append(f(((AmbientParticle) p).probability())).append(',');
            }
            return out.toString();
        }
        return "data";
    }

    static String name(EnvironmentAttribute<?> attribute) {
        return BuiltInRegistries.ENVIRONMENT_ATTRIBUTE.getKey(attribute).getPath();
    }

    static void attributes() {
        for (EnvironmentAttribute<?> a : BuiltInRegistries.ENVIRONMENT_ATTRIBUTE) {
            System.out.println("attr " + name(a) + " " + BuiltInRegistries.ATTRIBUTE_TYPE.getKey(a.type()) + " " + a.isSyncable() + " " + a.isPositional() + " "
                + a.isSpatiallyInterpolated() + " " + a.isFullResolutionBiomes() + " " + fmt(a.defaultValue()));
        }
    }

    static void line(String label, Object value) {
        System.out.println("mod " + label + " " + fmt(value));
    }

    static void modifiers() {
        float[] fs = {0.3F, -2.5F, 15.0F};
        float[] fa = {0.75F, 4.0F, -0.0F};
        String[] floatOps = {"add", "subtract", "multiply", "minimum", "maximum"};
        FloatModifier<?>[] floatMods = {FloatModifier.ADD, FloatModifier.SUBTRACT, FloatModifier.MULTIPLY, FloatModifier.MINIMUM, FloatModifier.MAXIMUM};
        for (int o = 0; o < floatOps.length; o++) {
            for (int i = 0; i < fs.length; i++) {
                for (int j = 0; j < fa.length; j++) {
                    line("float " + floatOps[o] + " " + i + " " + j, ((FloatModifier<Float>) floatMods[o]).apply(fs[i], fa[j]));
                }
            }
        }
        FloatWithAlpha[] alphas = {new FloatWithAlpha(4.0F, 0.3125F), new FloatWithAlpha(0.24F, 0.52734375F), new FloatWithAlpha(7.0F)};
        for (int i = 0; i < fs.length; i++) {
            for (int j = 0; j < alphas.length; j++) {
                line("float alpha_blend " + i + " " + j, FloatModifier.ALPHA_BLEND.apply(fs[i], alphas[j]));
            }
        }
        BooleanModifier[] bools = BooleanModifier.values();
        for (BooleanModifier m : bools) {
            for (boolean s : new boolean[]{false, true}) {
                for (boolean a : new boolean[]{false, true}) {
                    line("bool " + m.name().toLowerCase() + " " + s + " " + a, m.apply(s, a));
                }
            }
        }
        int[] is = {7, -3, Integer.MAX_VALUE};
        int[] ia = {2, -5};
        IntegerModifier<?>[] intMods = {IntegerModifier.ADD, IntegerModifier.SUBTRACT, IntegerModifier.MULTIPLY, IntegerModifier.MINIMUM, IntegerModifier.MAXIMUM};
        for (int o = 0; o < floatOps.length; o++) {
            for (int i = 0; i < is.length; i++) {
                for (int j = 0; j < ia.length; j++) {
                    line("int " + floatOps[o] + " " + i + " " + j, ((IntegerModifier<Integer>) intMods[o]).apply(is[i], ia[j]));
                }
            }
        }
        int nightSkyLight = ARGB.colorFromFloat(1.0F, 0.48F, 0.48F, 1.0F);
        int[] rgbSubjects = {0x78a7ff, 0xc0d8ff, 0x000000};
        int[] argbArgs = {ARGB.color(0.3125F, nightSkyLight), 0x80ff0000, 0x00123456, 0xff00ff00, 0x40404040};
        int[] rgbArgs = {0x7a7aff, 0xffffff, 0x0f0f0f};
        float[][] grays = {{0.6F, 0.75F}, {0.24F, 0.94F}, {0.095F, 0.94F}};
        int[] argbSubjects = {0xccffffff, 0x5fefc033, 0x00000000, 0xffffffff};
        for (int i = 0; i < rgbSubjects.length; i++) {
            Vector3fc s = ARGB.vector3fFromRGB24(rgbSubjects[i]);
            for (int j = 0; j < argbArgs.length; j++) {
                line("rgb alpha_blend " + i + " " + j, ColorModifier.ALPHA_BLEND_RGB.apply(s, ARGB.vector4fFromARGB32(argbArgs[j])));
            }
            for (int j = 0; j < rgbArgs.length; j++) {
                Vector3fc a = ARGB.vector3fFromRGB24(rgbArgs[j]);
                line("rgb add " + i + " " + j, ColorModifier.ADD_RGB.apply(s, a));
                line("rgb subtract " + i + " " + j, ColorModifier.SUBTRACT_RGB.apply(s, a));
                line("rgb multiply " + i + " " + j, ColorModifier.MULTIPLY_RGB.apply(s, a));
            }
            for (int j = 0; j < grays.length; j++) {
                line("rgb blend_to_gray " + i + " " + j, ColorModifier.BLEND_TO_GRAY_RGB.apply(s, new ColorModifier.BlendToGray(grays[j][0], grays[j][1])));
            }
        }
        for (int i = 0; i < argbSubjects.length; i++) {
            Vector4fc s = ARGB.vector4fFromARGB32(argbSubjects[i]);
            for (int j = 0; j < argbArgs.length; j++) {
                Vector4fc a = ARGB.vector4fFromARGB32(argbArgs[j]);
                line("argb alpha_blend " + i + " " + j, ColorModifier.ALPHA_BLEND_ARGB.apply(s, a));
                line("argb multiply " + i + " " + j, ColorModifier.MULTIPLY_ARGB.apply(s, a));
            }
            for (int j = 0; j < rgbArgs.length; j++) {
                Vector3fc a = ARGB.vector3fFromRGB24(rgbArgs[j]);
                line("argb add " + i + " " + j, ColorModifier.ADD_ARGB.apply(s, a));
                line("argb subtract " + i + " " + j, ColorModifier.SUBTRACT_ARGB.apply(s, a));
            }
            for (int j = 0; j < grays.length; j++) {
                line("argb blend_to_gray " + i + " " + j, ColorModifier.BLEND_TO_GRAY_ARGB.apply(s, new ColorModifier.BlendToGray(grays[j][0], grays[j][1])));
            }
        }
        float[][] degrees = {{0.25F, 350.0F, 10.0F}, {0.5F, 0.0F, 170.0F}, {0.5F, 10.0F, 200.0F}, {0.75F, -90.0F, 45.0F}, {0.3F, 720.0F, 30.0F}};
        for (int i = 0; i < degrees.length; i++) {
            line("lerp degrees " + i, AttributeTypes.ANGLE_DEGREES.partialTickLerp().apply(degrees[i][0], degrees[i][1], degrees[i][2]));
            line("lerp float " + i, AttributeTypes.FLOAT.keyframeLerp().apply(degrees[i][0], degrees[i][1], degrees[i][2]));
        }
        float[][] ints = {{0.5F, 3, 10}, {0.3F, 10, -7}, {0.99F, 0, 1}};
        for (int i = 0; i < ints.length; i++) {
            line("lerp int " + i, AttributeTypes.INTEGER.keyframeLerp().apply(ints[i][0], (int) ints[i][1], (int) ints[i][2]));
        }
        line("lerp rgb", AttributeTypes.RGB_COLOR.keyframeLerp().apply(0.37F, ARGB.vector3fFromRGB24(0x78a7ff), ARGB.vector3fFromRGB24(0x302821)));
        line("lerp argb", AttributeTypes.ARGB_COLOR.keyframeLerp().apply(0.61F, ARGB.vector4fFromARGB32(0xccffffff), ARGB.vector4fFromARGB32(0x5fefc033)));
        for (float alpha : new float[]{0.0F, 0.49F, 0.5F, 0.99F, 1.0F}) {
            line("lerp keyframe " + alpha, AttributeTypes.BOOLEAN.keyframeLerp().apply(alpha, false, true));
            line("lerp state_change " + alpha, AttributeTypes.BOOLEAN.stateChangeLerp().apply(alpha, false, true));
            line("lerp spatial " + alpha, AttributeTypes.BOOLEAN.spatialLerp().apply(alpha, false, true));
        }
        String[] biomes = {"plains", "desert", "mushroom_fields", "deep_dark", "soul_sand_valley", "the_void", "warm_ocean"};
        for (String a : biomes) {
            for (String b : biomes) {
                line("overlay " + a + " " + b, MobSpawnSettingsModifier.overlay().apply(spawns(a), spawns(b)));
            }
        }
    }

    static MobSpawnSettings spawns(String biome) {
        Biome b = registries.lookupOrThrow(Registries.BIOME).getOrThrow(net.minecraft.resources.ResourceKey.create(Registries.BIOME, Identifier.withDefaultNamespace(biome))).value();
        EnvironmentAttributeMap.Entry<MobSpawnSettings, ?> entry = b.getAttributes().get(EnvironmentAttributes.NATURAL_MOB_SPAWNS);
        return entry == null ? MobSpawnSettings.EMPTY : entry.applyModifier(MobSpawnSettings.EMPTY);
    }

    static void easing() {
        String[] names = {
            "constant", "linear", "in_back", "in_bounce", "in_circ", "in_cubic", "in_elastic", "in_expo", "in_quad", "in_quart", "in_quint", "in_sine",
            "in_out_back", "in_out_bounce", "in_out_circ", "in_out_cubic", "in_out_elastic", "in_out_expo", "in_out_quad", "in_out_quart", "in_out_quint",
            "in_out_sine", "out_back", "out_bounce", "out_circ", "out_cubic", "out_elastic", "out_expo", "out_quad", "out_quart", "out_quint", "out_sine"
        };
        float[] xs = {0.0F, 0.1F, 0.25F, 0.5F, 0.73F, 0.9F, 1.0F};
        for (String name : names) {
            EasingType easing = EasingType.CODEC.parse(JsonOps.INSTANCE, new JsonPrimitive(name)).getOrThrow();
            for (float x : xs) {
                System.out.println("ease " + name + " " + f(x) + " " + f(easing.apply(x)));
            }
        }
        EasingType bezier = EasingType.symmetricCubicBezier(0.362F, 0.241F);
        for (float x : xs) {
            System.out.println("ease bezier " + f(x) + " " + f(bezier.apply(x)));
        }
    }

    static long ticks;
    static float rain;
    static float thunder;

    static final ClockManager CLOCKS = clock -> new ClockInstance() {
        @Override
        public long totalTicks() {
            return ticks;
        }

        @Override
        public float partialTick() {
            return 0.0F;
        }

        @Override
        public float rate() {
            return 1.0F;
        }

        @Override
        public boolean isPaused() {
            return false;
        }
    };

    static BiomeManager biomeManager() {
        Holder<NoiseGeneratorSettings> settings = registries.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        Holder<MultiNoiseBiomeSourceParameterList> preset = registries
            .lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
            .getOrThrow(MultiNoiseBiomeSourceParameterLists.OVERWORLD);
        MultiNoiseBiomeSource source = MultiNoiseBiomeSource.createFromPreset(preset);
        RandomState randomState = RandomState.create(registries.lookupOrThrow(Registries.NOISE), SEED, settings.value());
        BiomeResolver resolver = source.createUncachedResolver(randomState);
        return new BiomeManager(resolver, BiomeManager.obfuscateSeed(SEED));
    }

    static EnvironmentAttributeSystem system(DimensionType type, boolean canHaveWeather, BiomeManager biomes) throws Exception {
        EnvironmentAttributeSystem.Builder builder = EnvironmentAttributeSystem.builder();
        builder.addConstantLayer(type.attributes());
        Method addBiomeLayer = EnvironmentAttributeSystem.class.getDeclaredMethod("addBiomeLayer", EnvironmentAttributeSystem.Builder.class, HolderLookup.class, BiomeManager.class);
        addBiomeLayer.setAccessible(true);
        addBiomeLayer.invoke(null, builder, registries.lookupOrThrow(Registries.BIOME), biomes);
        for (Holder<Timeline> timeline : type.timelines()) {
            builder.addTimelineLayer(timeline, CLOCKS);
        }
        if (canHaveWeather) {
            WeatherAttributes.addBuiltinLayers(builder, new WeatherAttributes.WeatherAccess() {
                @Override
                public float rainLevel() {
                    return rain;
                }

                @Override
                public float thunderLevel() {
                    return thunder;
                }
            });
        }
        return builder.build();
    }

    static void systems() throws Exception {
        BiomeManager biomes = biomeManager();
        long[] times = {0, 133, 1000, 6000, 12000, 12600, 13000, 13670, 18000, 23460, 23999, 30000, 100000, 191000};
        float[][] weathers = {{0.0F, 0.0F}, {0.4F, 0.0F}, {1.0F, 0.6F}};
        Object[][] dimensions = {{BuiltinDimensionTypes.OVERWORLD, true}, {BuiltinDimensionTypes.NETHER, false}, {BuiltinDimensionTypes.END, false}};
        for (Object[] dimension : dimensions) {
            var key = (net.minecraft.resources.ResourceKey<DimensionType>) dimension[0];
            boolean canHaveWeather = (Boolean) dimension[1];
            DimensionType type = registries.lookupOrThrow(Registries.DIMENSION_TYPE).getOrThrow(key).value();
            EnvironmentAttributeSystem system = system(type, canHaveWeather, biomes);
            for (long t : times) {
                for (float[] weather : canHaveWeather ? weathers : new float[][]{{0.0F, 0.0F}}) {
                    ticks = t;
                    rain = weather[0];
                    thunder = weather[1];
                    system.invalidateTickCache();
                    for (EnvironmentAttribute<?> a : BuiltInRegistries.ENVIRONMENT_ATTRIBUTE) {
                        String value = fmt(system.getDimensionValue(a));
                        if (!value.equals(fmt(a.defaultValue()))) {
                            System.out.println("dim " + key.identifier().getPath() + " " + t + " " + f(rain) + " " + f(thunder) + " " + name(a) + " " + value);
                        }
                    }
                }
            }
        }
    }

    static void positions() throws Exception {
        BiomeManager biomes = biomeManager();
        DimensionType type = registries.lookupOrThrow(Registries.DIMENSION_TYPE).getOrThrow(BuiltinDimensionTypes.OVERWORLD).value();
        EnvironmentAttributeSystem system = system(type, true, biomes);
        ticks = 13000;
        rain = 0.0F;
        thunder = 0.0F;
        system.invalidateTickCache();
        Map<String, int[]> found = new TreeMap<>();
        for (int y : new int[]{70, -40, 140}) {
            for (int r = 0; r <= 6000; r += 96) {
                for (int x = -r; x <= r; x += 96) {
                    for (int z : new int[]{-r, r}) {
                        String biome = biomes.getNoiseBiomeAtPosition(x, y, z).getRegisteredName();
                        found.putIfAbsent(biome, new int[]{x, y, z});
                    }
                }
            }
        }
        List<EnvironmentAttribute<?>> positional = new ArrayList<>();
        registries.lookupOrThrow(Registries.BIOME).listElements().forEach(biome -> {
            for (EnvironmentAttribute<?> a : biome.value().getAttributes().keySet()) {
                if (!positional.contains(a)) {
                    positional.add(a);
                }
            }
        });
        for (int[] p : found.values()) {
            for (int dx : new int[]{0, 1}) {
                Vec3 pos = new Vec3(p[0] + dx + 0.5, p[1] + 0.5, p[2] + 0.5);
                String at = "pos " + (p[0] + dx) + " " + p[1] + " " + p[2] + " ";
                System.out.println(at + "biome " + biomes.getNoiseBiomeAtPosition(pos.x, pos.y, pos.z).unwrapKey().orElseThrow().identifier().getPath());
                for (EnvironmentAttribute<?> a : positional) {
                    String value = fmt(system.getValue(a, pos));
                    if (!value.equals(fmt(a.defaultValue()))) {
                        System.out.println(at + name(a) + " " + (value.startsWith("mobs:") ? "mobs#" + Integer.toHexString(value.hashCode()) : value));
                    }
                }
            }
        }
    }
}
