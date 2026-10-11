package crisp;

import java.io.DataOutputStream;
import java.io.BufferedOutputStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Function;
import java.util.function.Supplier;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.QuartPos;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.SectionPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.WorldGenRegion;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.CloseableResourceManager;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.StaticCache2D;
import net.minecraft.world.flag.FeatureFlagSet;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.StructureManager;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.ChunkGenerator;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.DataLayer;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.chunk.status.ChunkPyramid;
import net.minecraft.world.level.dimension.BuiltinDimensionTypes;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.lighting.LayerLightEventListener;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.ticks.TickContainerAccess;
import net.minecraft.world.ticks.WorldGenTickAccess;
import sun.misc.Unsafe;
import net.minecraft.world.level.levelgen.Aquifer;
import net.minecraft.world.level.levelgen.Beardifier;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.densityfunction.DensityVolume;
import net.minecraft.world.level.levelgen.densityfunction.ScopedDensityBuffer;

public final class WorldgenOracle {
    private static final LevelHeightAccessor HEIGHT = LevelHeightAccessor.create(-64, 384);
    private static final Heightmap.Types[] FINAL = {
        Heightmap.Types.WORLD_SURFACE, Heightmap.Types.OCEAN_FLOOR, Heightmap.Types.MOTION_BLOCKING, Heightmap.Types.MOTION_BLOCKING_NO_LEAVES
    };

    private static RegistryAccess.Frozen registries;
    private static PalettedContainerFactory containers;
    private static CloseableResourceManager resources;
    private static RegistryAccess.Frozen fullRegistries;

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        loadRegistries();
        switch (args[0]) {
            case "terrain", "carved" -> terrain(args);
            case "features" -> features(args);
            case "structures" -> structures(args);
            case "find" -> find(args);
            case "spawn" -> spawn(args);
            case "bench" -> bench(args);
            default -> throw new IllegalArgumentException(args[0]);
        }
    }

    private static void loadRegistries() {
        PackRepository packs = ServerPacksSource.createVanillaTrustedRepository();
        packs.reload();
        packs.setSelected(List.of("vanilla"));
        resources = new MultiPackResourceManager(PackType.SERVER_DATA, packs.openAllSelected());
        LayeredRegistryAccess<RegistryLayer> layers = RegistryLayer.createRegistryAccess();
        List<Registry.PendingTags<?>> staticTags = TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC));
        RegistryAccess.Frozen context = layers.getAccessForLoading(RegistryLayer.WORLD);
        List<HolderLookup.RegistryLookup<?>> lookups = TagLoader.buildUpdatedLookups(context, staticTags);
        registries = RegistryDataLoader.load(resources, lookups, RegistryDataLoader.WORLD_REGISTRIES, Runnable::run).join();
        staticTags.forEach(Registry.PendingTags::apply);
        fullRegistries = layers.replaceFrom(RegistryLayer.WORLD, registries).compositeAccess();
        net.minecraft.core.registries.BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(fullRegistries)
            .forEach(net.minecraft.core.component.DataComponentInitializers.PendingComponents::apply);
        containers = PalettedContainerFactory.create(registries);
    }

    private record World(long seed, NoiseBasedChunkGenerator generator, RandomState randomState, Holder<NoiseGeneratorSettings> settings) {
    }

    private static World world(long seed) {
        Holder<NoiseGeneratorSettings> settings = registries.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        Holder<MultiNoiseBiomeSourceParameterList> preset = registries
            .lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
            .getOrThrow(MultiNoiseBiomeSourceParameterLists.OVERWORLD);
        NoiseBasedChunkGenerator generator = new NoiseBasedChunkGenerator(MultiNoiseBiomeSource.createFromPreset(preset), settings);
        RandomState randomState = RandomState.create(registries.lookupOrThrow(Registries.NOISE), seed, settings.value());
        return new World(seed, generator, randomState, settings);
    }

    private static Object call(Class<?> owner, String name, Object target, Class<?>[] types, Object... args) throws Exception {
        Method method = owner.getDeclaredMethod(name, types);
        method.setAccessible(true);
        return method.invoke(target, args);
    }

    private static ProtoChunk biomes(World world, ChunkPos pos) throws Exception {
        ProtoChunk chunk = new ProtoChunk(pos, UpgradeData.EMPTY, HEIGHT, containers, null);
        call(ChunkGenerator.class, "doCreateBiomes", world.generator, new Class<?>[]{Blender.class, RandomState.class, ChunkAccess.class},
            Blender.empty(), world.randomState, chunk);
        chunk.setPersistedStatus(net.minecraft.world.level.chunk.status.ChunkStatus.BIOMES);
        return chunk;
    }

    private static ProtoChunk terrain(World world, ChunkPos pos, Map<Long, ProtoChunk> biomeChunks) throws Exception {
        return terrain(world, pos, biomeChunks, false);
    }

    private static ProtoChunk terrain(World world, ChunkPos pos, Map<Long, ProtoChunk> biomeChunks, boolean carvers) throws Exception {
        return terrain(world, pos, biomeChunks, carvers, null);
    }

    private static ProtoChunk terrain(World world, ChunkPos pos, Map<Long, ProtoChunk> biomeChunks, boolean carvers, Beardifier beardifier) throws Exception {
        ProtoChunk[] region = new ProtoChunk[9];
        for (int dz = -1; dz <= 1; dz++) {
            for (int dx = -1; dx <= 1; dx++) {
                ChunkPos p = new ChunkPos(pos.x() + dx, pos.z() + dz);
                ProtoChunk c = biomeChunks.get(p.pack());
                if (c == null) {
                    c = biomes(world, p);
                    biomeChunks.put(p.pack(), c);
                }
                region[(dz + 1) * 3 + dx + 1] = c;
            }
        }
        ProtoChunk chunk = region[4];
        Set<Holder<Biome>> possible = new HashSet<>();
        for (ProtoChunk c : region) {
            c.collectBiomesInPalette(possible);
        }
        BiomeManager biomeManager = new BiomeManager((qx, qy, qz) -> {
            int cx = QuartPos.toSection(qx) - pos.x();
            int cz = QuartPos.toSection(qz) - pos.z();
            return region[(cz + 1) * 3 + cx + 1].getNoiseBiome(qx, qy, qz);
        }, BiomeManager.obfuscateSeed(world.seed));

        NoiseGeneratorSettings settings = world.settings.value();
        NoiseSettings noiseSettings = settings.noiseSettings().clampToHeightAccessor(chunk.getHeightAccessorForGeneration());
        Field pickerField = NoiseBasedChunkGenerator.class.getDeclaredField("globalFluidPicker");
        pickerField.setAccessible(true);
        @SuppressWarnings("unchecked")
        Aquifer.FluidPicker picker = ((Supplier<Aquifer.FluidPicker>) pickerField.get(world.generator)).get();
        DensityVolume volume = new DensityVolume(16, noiseSettings.height(), 16, pos.getMinBlockX(), noiseSettings.minY(), pos.getMinBlockZ());
        try (NoiseChunk noiseChunk = new NoiseChunk(world.randomState, beardifier, settings, picker, Blender.empty(), volume)) {
            call(NoiseBasedChunkGenerator.class, "doFill", world.generator, new Class<?>[]{NoiseChunk.class, ChunkAccess.class}, noiseChunk, chunk);
            call(NoiseBasedChunkGenerator.class, "buildSurface", world.generator,
                new Class<?>[]{ChunkAccess.class, NoiseChunk.class, RandomState.class, BiomeManager.class, Set.class,
                    net.minecraft.world.level.levelgen.material.rule.MaterialRule.class},
                chunk, noiseChunk, world.randomState, biomeManager, possible, settings.materialRule().value());
            if (carvers) {
                call(NoiseBasedChunkGenerator.class, "generateCarvers", world.generator,
                    new Class<?>[]{ChunkAccess.class, Blender.class, NoiseChunk.class, RandomState.class, BiomeManager.class, WorldGenRegion.class,
                        net.minecraft.world.level.levelgen.material.rule.MaterialRule.class},
                    chunk, Blender.empty(), noiseChunk, world.randomState, biomeManager, null, settings.materialRule().value());
            }
        }
        Heightmap.primeHeightmaps(chunk, EnumSet.of(Heightmap.Types.MOTION_BLOCKING, Heightmap.Types.MOTION_BLOCKING_NO_LEAVES,
            Heightmap.Types.OCEAN_FLOOR, Heightmap.Types.WORLD_SURFACE));
        return chunk;
    }

    private static long fnv(long hash, int value) {
        for (int i = 0; i < 4; i++) {
            hash ^= (value >>> (8 * i)) & 0xFF;
            hash *= 0x100000001b3L;
        }
        return hash;
    }

    private static long fnv(long hash, String value) {
        for (byte b : value.getBytes(StandardCharsets.UTF_8)) {
            hash ^= b & 0xFF;
            hash *= 0x100000001b3L;
        }
        hash ^= 0;
        hash *= 0x100000001b3L;
        return hash;
    }

    private static final long FNV_OFFSET = 0xcbf29ce484222325L;

    private static void terrain(String[] args) throws Exception {
        String dump = args[1];
        long seed = Long.parseLong(args[2]);
        World world = world(seed);
        Map<Long, ProtoChunk> biomeChunks = new LinkedHashMap<>();
        for (int i = 3; i + 1 < args.length; i += 2) {
            ChunkPos pos = new ChunkPos(Integer.parseInt(args[i]), Integer.parseInt(args[i + 1]));
            report(seed, pos, terrain(world, pos, biomeChunks, args[0].equals("carved")), dump);
        }
    }

    private static void report(long seed, ChunkPos pos, ProtoChunk chunk, String dump) throws Exception {
        {
            long blocks = FNV_OFFSET;
            int[] ids = new int[16 * 384 * 16];
            int n = 0;
            Map<String, Integer> counts = new LinkedHashMap<>();
            for (int y = -64; y < 320; y++) {
                for (int z = 0; z < 16; z++) {
                    for (int x = 0; x < 16; x++) {
                        BlockState state = chunk.getBlockState(new net.minecraft.core.BlockPos(pos.getMinBlockX() + x, y, pos.getMinBlockZ() + z));
                        int id = Block.BLOCK_STATE_REGISTRY.getId(state);
                        ids[n++] = id;
                        blocks = fnv(blocks, id);
                    }
                }
            }
            long biomes = FNV_OFFSET;
            List<String> biomeNames = new ArrayList<>();
            for (int qy = -16; qy < 80; qy++) {
                for (int qx = 0; qx < 4; qx++) {
                    for (int qz = 0; qz < 4; qz++) {
                        String name = chunk.getNoiseBiome(QuartPos.fromBlock(pos.getMinBlockX()) + qx, qy, QuartPos.fromBlock(pos.getMinBlockZ()) + qz)
                            .unwrapKey().orElseThrow().identifier().toString();
                        biomeNames.add(name);
                        biomes = fnv(biomes, name);
                    }
                }
            }
            long heights = FNV_OFFSET;
            for (Heightmap.Types type : FINAL) {
                for (int z = 0; z < 16; z++) {
                    for (int x = 0; x < 16; x++) {
                        heights = fnv(heights, chunk.getHeight(type, x, z));
                    }
                }
            }
            for (int id : ids) {
                String name = Block.BLOCK_STATE_REGISTRY.byId(id).getBlock().builtInRegistryHolder().key().identifier().getPath();
                if (name.startsWith("raw_") || name.endsWith("_ore") || name.equals("lava") || name.equals("packed_ice") || name.equals("snow_block")) {
                    counts.merge(name, 1, Integer::sum);
                }
            }
            Set<String> surfaceBiomes = new java.util.TreeSet<>();
            for (String biomeName : biomeNames) {
                surfaceBiomes.add(biomeName.substring("minecraft:".length()));
            }
            System.out.printf("%d %d %d %016x %016x %016x # %s %s%n", seed, pos.x(), pos.z(), blocks, biomes, heights, surfaceBiomes, counts);
            if (!dump.equals("-")) {
                Path dir = Path.of(dump);
                Files.createDirectories(dir);
                try (DataOutputStream out = new DataOutputStream(new BufferedOutputStream(Files.newOutputStream(dir.resolve(seed + "_" + pos.x() + "_" + pos.z() + ".blocks"))))) {
                    for (int id : ids) {
                        out.writeInt(id);
                    }
                }
                Files.write(dir.resolve(seed + "_" + pos.x() + "_" + pos.z() + ".biomes"), biomeNames);
                try (DataOutputStream out = new DataOutputStream(new BufferedOutputStream(Files.newOutputStream(dir.resolve(seed + "_" + pos.x() + "_" + pos.z() + ".heights"))))) {
                    for (Heightmap.Types type : FINAL) {
                        for (int z = 0; z < 16; z++) {
                            for (int x = 0; x < 16; x++) {
                                out.writeInt(chunk.getHeight(type, x, z));
                            }
                        }
                    }
                }
            }
        }
    }

    public static class OracleLevel extends ServerLevel {
        static RegistryAccess registryAccess;
        static DimensionType dimensionType;
        static LevelLightEngine lightEngine;
        static long seed;

        OracleLevel() {
            super(null, null, null, null, null, null, false, 0L, null, false);
        }

        @Override
        public long getSeed() {
            return seed;
        }

        @Override
        public RegistryAccess registryAccess() {
            return registryAccess;
        }

        @Override
        public DimensionType dimensionType() {
            return dimensionType;
        }

        @Override
        public LevelLightEngine getLightEngine() {
            return lightEngine;
        }

        @Override
        public net.minecraft.world.level.storage.LevelData getLevelData() {
            return new net.minecraft.world.level.storage.LevelData() {
                @Override
                public RespawnData getRespawnData() {
                    return RespawnData.DEFAULT;
                }

                @Override
                public long getGameTime() {
                    return 0L;
                }

                @Override
                public boolean isHardcore() {
                    return false;
                }

                @Override
                public net.minecraft.world.Difficulty getDifficulty() {
                    return net.minecraft.world.Difficulty.PEACEFUL;
                }

                @Override
                public boolean isDifficultyLocked() {
                    return false;
                }
            };
        }

        @Override
        public void updatePOIOnBlockStateChange(final BlockPos pos, final BlockState oldState, final BlockState newState) {
        }

        @Override
        public int getSeaLevel() {
            return 63;
        }

        @Override
        public FeatureFlagSet enabledFeatures() {
            boolean entity = System.getenv("ORACLE_STRUCTURES") != null
                && StackWalker.getInstance().walk(frames -> frames.limit(6).anyMatch(frame -> frame.getClassName().startsWith("net.minecraft.world.entity.")));
            return entity ? FeatureFlagSet.of() : FeatureFlags.DEFAULT_FLAGS;
        }

        static net.minecraft.server.level.ServerChunkCache chunkSource;

        @Override
        public net.minecraft.server.level.ServerChunkCache getChunkSource() {
            return chunkSource;
        }
    }

    public static class OracleChunkCache extends net.minecraft.server.level.ServerChunkCache {
        static net.minecraft.world.level.chunk.ChunkGenerator generator;

        OracleChunkCache() {
            super(null, null, null, null, null, null, 0, 0, false, null);
        }

        @Override
        public net.minecraft.world.level.chunk.ChunkGenerator getGenerator() {
            return generator;
        }
    }

    private static final class UnlitLightEngine extends LevelLightEngine {
        UnlitLightEngine() {
            super(new net.minecraft.world.level.chunk.LightChunkGetter() {
                @Override
                public net.minecraft.world.level.chunk.LightChunk getChunkForLighting(int x, int z) {
                    return null;
                }

                @Override
                public net.minecraft.world.level.BlockGetter getLevel() {
                    return null;
                }
            }, false, false);
        }

        private static LayerLightEventListener constant(int value) {
            return new LayerLightEventListener() {
                @Override
                public DataLayer getDataLayerData(SectionPos pos) {
                    return null;
                }

                @Override
                public int getLightValue(net.minecraft.core.BlockPos pos) {
                    return value;
                }

                @Override
                public void checkBlock(net.minecraft.core.BlockPos pos) {
                }

                @Override
                public boolean hasLightWork() {
                    return false;
                }

                @Override
                public int runLightUpdates() {
                    return 0;
                }

                @Override
                public void updateSectionStatus(SectionPos pos, boolean sectionEmpty) {
                }

                @Override
                public void setLightEnabled(ChunkPos pos, boolean enable) {
                }

                @Override
                public void propagateLightSources(ChunkPos pos) {
                }
            };
        }

        @Override
        public LayerLightEventListener getLayerListener(LightLayer layer) {
            return constant(layer == LightLayer.SKY ? 15 : 0);
        }

        @Override
        public int getRawBrightness(net.minecraft.core.BlockPos pos, int skyDampen) {
            return Math.max(15 - skyDampen, 0);
        }
    }

    private static void setField(Object target, String name, Object value) throws Exception {
        for (Class<?> c = target.getClass(); c != null; c = c.getSuperclass()) {
            try {
                Field field = c.getDeclaredField(name);
                field.setAccessible(true);
                field.set(target, value);
                return;
            } catch (NoSuchFieldException e) {
            }
        }
        throw new NoSuchFieldException(name);
    }

    private static net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager templateManager() throws Exception {
        net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager templates =
            (net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager) unsafe().allocateInstance(
                net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager.class
            );
        setField(templates, "structureRepository", new java.util.concurrent.ConcurrentHashMap<>());
        setField(templates, "sources", List.of(new net.minecraft.world.level.levelgen.structure.templatesystem.loader.ResourceManagerTemplateSource(
            net.minecraft.util.datafix.DataFixers.getDataFixer(),
            net.minecraft.core.registries.BuiltInRegistries.BLOCK,
            resources,
            new net.minecraft.resources.FileToIdConverter("structure", ".nbt")
        )));
        return templates;
    }

    private static void structures(String[] args) throws Exception {
        long seed = Long.parseLong(args[1]);
        int fromX = Integer.parseInt(args[2]);
        int fromZ = Integer.parseInt(args[3]);
        int toX = Integer.parseInt(args[4]);
        int toZ = Integer.parseInt(args[5]);
        World world = world(seed);
        var templates = templateManager();
        var state = world.generator.createState(registries.lookupOrThrow(Registries.STRUCTURE_SET), world.randomState, seed);
        StructureManager structures = new StructureManager(null, new WorldOptions(seed, true, false), null);
        var context = new net.minecraft.world.level.levelgen.structure.pieces.StructurePieceSerializationContext(resources, fullRegistries, templates);
        for (int x = fromX; x <= toX; x++) {
            for (int z = fromZ; z <= toZ; z++) {
                ChunkPos pos = new ChunkPos(x, z);
                ProtoChunk chunk = new ProtoChunk(pos, UpgradeData.EMPTY, HEIGHT, containers, null);
                world.generator.createStructures(fullRegistries, state, structures, chunk, templates, net.minecraft.world.level.Level.OVERWORLD);
                for (var entry : chunk.getAllStarts().entrySet()) {
                    String id = registries.lookupOrThrow(Registries.STRUCTURE).getKey(entry.getKey()).toString();
                    System.out.println("START " + x + " " + z + " " + id + " " + entry.getValue().createTag(context, pos));
                }
            }
        }
    }

    private static Unsafe unsafe() throws Exception {
        Field theUnsafe = Unsafe.class.getDeclaredField("theUnsafe");
        theUnsafe.setAccessible(true);
        return (Unsafe) theUnsafe.get(null);
    }

    private static WorldGenRegion region(World world, ServerLevel level, ChunkPos center, Map<Long, ProtoChunk> chunks, java.util.function.LongFunction<ProtoChunk> outer) throws Exception {
        WorldGenRegion region = (WorldGenRegion) unsafe().allocateInstance(WorldGenRegion.class);
        StaticCache2D<ChunkAccess> cache = StaticCache2D.create(center.x(), center.z(), 8, (x, z) -> {
            ProtoChunk chunk = chunks.get(ChunkPos.pack(x, z));
            return chunk != null ? chunk : outer.apply(ChunkPos.pack(x, z));
        });
        setField(region, "cache", cache);
        setField(region, "center", cache.get(center.x(), center.z()));
        setField(region, "level", level);
        setField(region, "seed", world.seed);
        setField(region, "levelData", new net.minecraft.world.level.storage.LevelData() {
            @Override
            public RespawnData getRespawnData() {
                return RespawnData.DEFAULT;
            }

            @Override
            public long getGameTime() {
                return 0L;
            }

            @Override
            public boolean isHardcore() {
                return false;
            }

            @Override
            public net.minecraft.world.Difficulty getDifficulty() {
                return net.minecraft.world.Difficulty.NORMAL;
            }

            @Override
            public boolean isDifficultyLocked() {
                return false;
            }
        });
        setField(region, "random", world.randomState.getOrCreateRandomFactory(Identifier.withDefaultNamespace("worldgen_region_random")).at(center.getWorldPosition()));
        setField(region, "dimensionType", OracleLevel.dimensionType);
        Function<net.minecraft.core.BlockPos, TickContainerAccess<Block>> blockTicks = pos -> region.getChunk(pos).getBlockTicks();
        Function<net.minecraft.core.BlockPos, TickContainerAccess<Fluid>> fluidTicks = pos -> region.getChunk(pos).getFluidTicks();
        setField(region, "blockTicks", new WorldGenTickAccess<>(blockTicks));
        setField(region, "fluidTicks", new WorldGenTickAccess<>(fluidTicks));
        setField(region, "biomeManager", new BiomeManager(region, BiomeManager.obfuscateSeed(world.seed)));
        setField(region, "generatingStep", ChunkPyramid.GENERATION_PYRAMID.getStepTo(net.minecraft.world.level.chunk.status.ChunkStatus.FEATURES));
        setField(region, "subTickCount", new AtomicLong());
        setField(region, "uncachedBiomeResolver", world.generator.getBiomeSource().createUncachedResolver(world.randomState));
        setField(region, "centerChunkX", center.x());
        setField(region, "centerChunkZ", center.z());
        setField(region, "writeRadius", 1);
        return region;
    }

    private static void features(String[] args) throws Exception {
        String dump = args[1];
        long seed = Long.parseLong(args[2]);
        World world = world(seed);
        OracleLevel.registryAccess = fullRegistries;
        OracleLevel.dimensionType = registries.lookupOrThrow(Registries.DIMENSION_TYPE).getOrThrow(BuiltinDimensionTypes.OVERWORLD).value();
        OracleLevel.lightEngine = new UnlitLightEngine();
        OracleLevel.seed = seed;
        OracleChunkCache.generator = world.generator;
        String skip = System.getenv("ORACLE_SKIP_FEATURE_TYPES");
        if (skip != null) {
            Set<String> skipped = Set.of(skip.split(","));
            Field value = Holder.Reference.class.getDeclaredField("value");
            value.setAccessible(true);
            for (Holder.Reference<net.minecraft.world.level.levelgen.feature.Feature> holder : registries.lookupOrThrow(Registries.FEATURE).listElements().toList()) {
                String type = net.minecraft.core.registries.BuiltInRegistries.FEATURE_TYPE.getKey(holder.value().codec()).toString();
                if (skipped.contains(type)) {
                    value.set(holder, new net.minecraft.world.level.levelgen.feature.NoOpFeature());
                }
            }
        }
        OracleLevel.chunkSource = (OracleChunkCache) unsafe().allocateInstance(OracleChunkCache.class);
        ServerLevel level = (ServerLevel) unsafe().allocateInstance(OracleLevel.class);
        Object server = unsafe().allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        var templates = templateManager();
        setField(server, "structureTemplateManager", templates);
        setField(level, "server", server);
        boolean withStructures = System.getenv("ORACLE_STRUCTURES") != null;
        WorldOptions worldOptions = new WorldOptions(seed, withStructures, false);
        StructureManager structures = new StructureManager(null, worldOptions, null);
        var structureState = world.generator.createState(registries.lookupOrThrow(Registries.STRUCTURE_SET), world.randomState, seed);
        Map<Long, ProtoChunk> structureChunks = new java.util.HashMap<>();
        java.util.function.LongFunction<ProtoChunk> startsChunk = key -> structureChunks.computeIfAbsent(key, k -> {
            ProtoChunk c = new ProtoChunk(ChunkPos.unpack(k), UpgradeData.EMPTY, HEIGHT, containers, null);
            if (withStructures) {
                world.generator.createStructures(fullRegistries, structureState, structures, c, templates, net.minecraft.world.level.Level.OVERWORLD);
                String only = System.getenv("ORACLE_STRUCTURE_TYPES");
                if (only != null) {
                    Set<String> allowed = Set.of(only.split(","));
                    var kept = new java.util.HashMap<>(c.getAllStarts());
                    kept.keySet().removeIf(s -> !allowed.contains(net.minecraft.core.registries.BuiltInRegistries.STRUCTURE_TYPE.getKey(s.type()).toString()));
                    c.setAllStarts(kept);
                }
            }
            return c;
        });
        Map<Long, ProtoChunk> biomeChunks = new LinkedHashMap<>();
        Map<Long, ProtoChunk> terrainChunks = new LinkedHashMap<>();
        Set<Long> decorated = new HashSet<>();
        Set<Long> referenced = new HashSet<>();
        for (int i = 3; i + 1 < args.length; i += 2) {
            ChunkPos pos = new ChunkPos(Integer.parseInt(args[i]), Integer.parseInt(args[i + 1]));
            for (int dz = -1; dz <= 1; dz++) {
                for (int dx = -1; dx <= 1; dx++) {
                    ChunkPos center = new ChunkPos(pos.x() + dx, pos.z() + dz);
                    if (!decorated.add(center.pack())) {
                        continue;
                    }
                    for (int nz = -1; nz <= 1; nz++) {
                        for (int nx = -1; nx <= 1; nx++) {
                            ChunkPos p = new ChunkPos(center.x() + nx, center.z() + nz);
                            if (!terrainChunks.containsKey(p.pack())) {
                                ProtoChunk startsOnly = startsChunk.apply(p.pack());
                                if (referenced.add(p.pack())) {
                                    for (int sx = p.x() - 8; sx <= p.x() + 8; sx++) {
                                        for (int sz = p.z() - 8; sz <= p.z() + 8; sz++) {
                                            long sourceKey = ChunkPos.pack(sx, sz);
                                            for (var start : startsChunk.apply(sourceKey).getAllStarts().values()) {
                                                if (start.isValid() && start.getBoundingBox().intersects(p.getMinBlockX(), p.getMinBlockZ(), p.getMaxBlockX(), p.getMaxBlockZ())) {
                                                    startsOnly.addReferenceForStructure(start.getStructure(), sourceKey);
                                                }
                                            }
                                        }
                                    }
                                }
                                WorldGenRegion beardRegion = region(world, level, p, Map.of(), startsChunk);
                                Beardifier beardifier = withStructures
                                    ? Beardifier.forStructuresInChunk(new StructureManager(beardRegion, worldOptions, null), p)
                                    : null;
                                ProtoChunk chunk = terrain(world, p, biomeChunks, true, beardifier);
                                chunk.setPersistedStatus(net.minecraft.world.level.chunk.status.ChunkStatus.TERRAIN);
                                chunk.setAllStarts(startsOnly.getAllStarts());
                                chunk.setAllReferences(startsOnly.getAllReferences());
                                terrainChunks.put(p.pack(), chunk);
                            }
                        }
                    }
                    ProtoChunk chunk = terrainChunks.get(center.pack());
                    WorldGenRegion region = region(world, level, center, terrainChunks, startsChunk);
                    world.generator.applyBiomeDecoration(region, chunk, new StructureManager(region, worldOptions, null));
                    chunk.setPersistedStatus(net.minecraft.world.level.chunk.status.ChunkStatus.FEATURES);
                }
            }
            report(seed, pos, terrainChunks.get(pos.pack()), dump);
        }
    }

    private static void find(String[] args) throws Exception {
        long seed = Long.parseLong(args[1]);
        int radius = Integer.parseInt(args[2]);
        World world = world(seed);
        var resolver = world.generator.getBiomeSource().createUncachedResolver(world.randomState);
        Set<String> wanted = new HashSet<>(List.of(args).subList(3, args.length));
        for (int r = 0; r <= radius; r += 4) {
            for (int cx = -r; cx <= r; cx += 4) {
                for (int cz = -r; cz <= r; cz += 4) {
                    if (Math.max(Math.abs(cx), Math.abs(cz)) != r) {
                        continue;
                    }
                    for (int quartY : new int[]{QuartPos.fromBlock(72), QuartPos.fromBlock(-40)}) {
                        String name = resolver.getNoiseBiome(QuartPos.fromBlock(cx * 16 + 8), quartY, QuartPos.fromBlock(cz * 16 + 8))
                            .unwrapKey().orElseThrow().identifier().toString();
                        if (wanted.remove(name)) {
                            System.out.println(name + " " + cx + " " + cz);
                        }
                    }
                }
            }
        }
    }

    private static void bench(String[] args) throws Exception {
        World world = world(Long.parseLong(args[1]));
        for (int round = 0; round < 2; round++) {
            Map<Long, ProtoChunk> biomeChunks = new LinkedHashMap<>();
            long start = System.nanoTime();
            for (int x = 0; x < 20; x++) {
                for (int z = 0; z < 20; z++) {
                    terrain(world, new ChunkPos(x - 10 + round * 100, z - 10), biomeChunks);
                }
            }
            System.out.printf("round %d: %.0f chunks/s%n", round, 400 / ((System.nanoTime() - start) / 1e9));
        }
    }

    private static void spawn(String[] args) throws Exception {
        World world = world(Long.parseLong(args[1]));
        ChunkPos origin = world.generator.getOrigin(world.randomState);
        System.out.println("origin " + origin.x() + " " + origin.z());
    }
}
