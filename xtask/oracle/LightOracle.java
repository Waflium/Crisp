package crisp;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import java.security.MessageDigest;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Holder;
import net.minecraft.core.IdMap;
import net.minecraft.core.SectionPos;
import net.minecraft.network.protocol.game.ClientboundLightUpdatePacketData;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.SlabBlock;
import net.minecraft.world.level.block.StairBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.SlabType;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.LightChunk;
import net.minecraft.world.level.chunk.LightChunkGetter;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.Strategy;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.lighting.LightEngine;
import net.minecraft.world.level.material.FluidState;

public final class LightOracle {
    static final int MIN_Y = -64;
    static final int HEIGHT = 384;
    static PalettedContainerFactory factory;

    final List<BlockState> layers;
    final Map<BlockPos, BlockState> decorations = new LinkedHashMap<>();
    final Map<Long, ProtoChunk> chunks = new HashMap<>();
    final Set<Long> lit = new HashSet<>();
    final Map<Long, Boolean> lightCorrect = new HashMap<>();
    final ArrayDeque<Object[]> tasks = new ArrayDeque<>();
    final Level level = new Level();
    final LevelLightEngine engine;

    LightOracle(final List<BlockState> layers) {
        this.layers = layers;
        this.engine = new LevelLightEngine(new Getter(), true, true);
    }

    final class Level implements BlockGetter {
        @Override
        public BlockEntity getBlockEntity(final BlockPos pos) {
            return null;
        }

        @Override
        public BlockState getBlockState(final BlockPos pos) {
            ProtoChunk chunk = chunks.get(ChunkPos.pack(pos.getX() >> 4, pos.getZ() >> 4));
            return chunk == null ? Blocks.AIR.defaultBlockState() : chunk.getBlockState(pos);
        }

        @Override
        public FluidState getFluidState(final BlockPos pos) {
            return this.getBlockState(pos).getFluidState();
        }

        @Override
        public int getHeight() {
            return HEIGHT;
        }

        @Override
        public int getMinY() {
            return MIN_Y;
        }
    }

    final class Getter implements LightChunkGetter {
        @Override
        public LightChunk getChunkForLighting(final int x, final int z) {
            return chunks.get(ChunkPos.pack(x, z));
        }

        @Override
        public BlockGetter getLevel() {
            return level;
        }
    }

    static List<BlockState> flat() {
        List<BlockState> layers = new ArrayList<>();
        layers.add(Blocks.BEDROCK.defaultBlockState());
        layers.add(Blocks.DIRT.defaultBlockState());
        layers.add(Blocks.DIRT.defaultBlockState());
        layers.add(Blocks.GRASS_BLOCK.defaultBlockState());
        return layers;
    }

    ProtoChunk generate(final ChunkPos pos) {
        ProtoChunk chunk = new ProtoChunk(pos, UpgradeData.EMPTY, this.level, factory, null);
        for (int i = 0; i < this.layers.size(); i++) {
            int y = MIN_Y + i;
            LevelChunkSection section = chunk.getSection(chunk.getSectionIndex(y));
            for (int z = 0; z < 16; z++) {
                for (int x = 0; x < 16; x++) {
                    section.setBlockState(x, y & 15, z, this.layers.get(i));
                }
            }
        }
        for (Map.Entry<BlockPos, BlockState> e : this.decorations.entrySet()) {
            BlockPos p = e.getKey();
            if (p.getX() >> 4 == pos.x() && p.getZ() >> 4 == pos.z()) {
                chunk.getSection(chunk.getSectionIndex(p.getY())).setBlockState(p.getX() & 15, p.getY() & 15, p.getZ() & 15, e.getValue());
            }
        }
        return chunk;
    }

    void addTask(final boolean pre, final Runnable task) {
        this.tasks.add(new Object[]{pre, task});
        if (this.tasks.size() >= 1000) {
            this.runUpdate();
        }
    }

    void runUpdate() {
        int total = Math.min(this.tasks.size(), 1000);
        List<Object[]> batch = new ArrayList<>();
        for (int i = 0; i < total; i++) {
            batch.add(this.tasks.poll());
        }
        for (Object[] task : batch) {
            if ((Boolean)task[0]) {
                ((Runnable)task[1]).run();
            }
        }
        this.engine.runLightUpdates();
        for (Object[] task : batch) {
            if (!(Boolean)task[0]) {
                ((Runnable)task[1]).run();
            }
        }
    }

    void runAll() {
        while (!this.tasks.isEmpty() || this.engine.hasLightWork()) {
            this.runUpdate();
        }
    }

    boolean loadForLighting(final ChunkPos pos) {
        if (this.chunks.containsKey(pos.pack())) {
            return false;
        }
        ProtoChunk chunk = this.generate(pos);
        chunk.initializeLightSources();
        boolean lighted = false;
        this.lightCorrect.put(pos.pack(), false);
        this.chunks.put(pos.pack(), chunk);
        this.addTask(true, () -> {
            for (int i = 0; i < chunk.getSectionsCount(); i++) {
                if (!chunk.getSection(i).hasOnlyAir()) {
                    this.engine.updateSectionStatus(SectionPos.of(pos, chunk.getSectionYFromSectionIndex(i)), false);
                }
            }
        });
        this.addTask(false, () -> {
            this.engine.setLightEnabled(pos, lighted);
            this.engine.retainData(pos, false);
        });
        return true;
    }

    void lightChunks(final List<ChunkPos> positions) {
        boolean initialized = false;
        for (ChunkPos pos : positions) {
            for (int dz = -1; dz <= 1; dz++) {
                for (int dx = -1; dx <= 1; dx++) {
                    initialized |= this.loadForLighting(new ChunkPos(pos.x() + dx, pos.z() + dz));
                }
            }
        }
        if (initialized) {
            this.runAll();
        }
        for (ChunkPos pos : positions) {
            this.lit.add(pos.pack());
            boolean lighted = this.lightCorrect.get(pos.pack());
            this.lightCorrect.put(pos.pack(), false);
            this.addTask(true, () -> {
                if (!lighted) {
                    this.engine.propagateLightSources(pos);
                }
            });
            this.addTask(false, () -> this.lightCorrect.put(pos.pack(), true));
        }
    }

    void prepareTicking(final int x, final int z) {
        List<ChunkPos> unlit = new ArrayList<>();
        for (int dz = -1; dz <= 1; dz++) {
            for (int dx = -1; dx <= 1; dx++) {
                ChunkPos neighbour = new ChunkPos(x + dx, z + dz);
                if (!this.lit.contains(neighbour.pack())) {
                    unlit.add(neighbour);
                }
            }
        }
        if (!unlit.isEmpty()) {
            this.lightChunks(unlit);
            this.runAll();
        }
    }

    void setBlock(final int x, final int y, final int z, final BlockState state) {
        ChunkPos chunkPos = new ChunkPos(x >> 4, z >> 4);
        if (!this.lit.contains(chunkPos.pack())) {
            this.lightChunks(List.of(chunkPos));
            this.runAll();
        }
        ProtoChunk chunk = this.chunks.get(chunkPos.pack());
        LevelChunkSection section = chunk.getSection(chunk.getSectionIndex(y));
        boolean wasEmpty = section.hasOnlyAir();
        if (wasEmpty && state.isAir()) {
            return;
        }
        BlockState old = section.setBlockState(x & 15, y & 15, z & 15, state);
        if (old == state) {
            return;
        }
        boolean isEmpty = section.hasOnlyAir();
        boolean changed = LightEngine.hasDifferentLightProperties(old, state);
        if (changed) {
            chunk.getSkyLightSources().update(chunk, x & 15, y, z & 15);
        }
        BlockPos pos = new BlockPos(x, y, z);
        if (wasEmpty != isEmpty) {
            this.addTask(true, () -> this.engine.updateSectionStatus(SectionPos.of(pos), isEmpty));
        }
        if (changed) {
            this.addTask(true, () -> this.engine.checkBlock(pos));
        }
    }

    void dump(final String name, final int minX, final int maxX, final int minZ, final int maxZ, final int[][] probes) throws Exception {
        System.out.println("// " + name);
        for (int z = minZ; z <= maxZ; z++) {
            for (int x = minX; x <= maxX; x++) {
                ByteBuf buf = Unpooled.buffer();
                ClientboundLightUpdatePacketData.STREAM_CODEC.encode(buf, new ClientboundLightUpdatePacketData(new ChunkPos(x, z), this.engine, null, null));
                byte[] bytes = new byte[buf.readableBytes()];
                buf.readBytes(bytes);
                String hash = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
                System.out.println("((" + x + ", " + z + "), \"" + hash + "\"),");
            }
        }
        for (int[] p : probes) {
            BlockPos pos = new BlockPos(p[0], p[1], p[2]);
            int sky = this.engine.getLayerListener(LightLayer.SKY).getLightValue(pos);
            int block = this.engine.getLayerListener(LightLayer.BLOCK).getLightValue(pos);
            System.out.println("((" + p[0] + ", " + p[1] + ", " + p[2] + "), " + sky + ", " + block + "),");
        }
    }

    static BlockState slab(final SlabType type) {
        return Blocks.SMOOTH_STONE_SLAB.defaultBlockState().setValue(SlabBlock.TYPE, type);
    }

    static List<List<Object[]>> steps() {
        List<List<Object[]>> steps = new ArrayList<>();
        List<Object[]> torch = new ArrayList<>();
        torch.add(new Object[]{8, -60, 8, Blocks.TORCH.defaultBlockState()});
        steps.add(torch);
        List<Object[]> overhang = new ArrayList<>();
        for (int x = 13; x <= 17; x++) {
            for (int z = 6; z <= 10; z++) {
                overhang.add(new Object[]{x, -57, z, Blocks.STONE.defaultBlockState()});
            }
        }
        steps.add(overhang);
        List<Object[]> blockers = new ArrayList<>();
        blockers.add(new Object[]{3, -60, 3, Blocks.GLASS.defaultBlockState()});
        blockers.add(new Object[]{3, -59, 3, Blocks.GLASS.defaultBlockState()});
        blockers.add(new Object[]{5, -60, 3, Blocks.OAK_LEAVES.defaultBlockState()});
        blockers.add(new Object[]{5, -59, 3, Blocks.OAK_LEAVES.defaultBlockState()});
        blockers.add(new Object[]{5, -58, 3, Blocks.OAK_LEAVES.defaultBlockState()});
        blockers.add(new Object[]{7, -60, 3, Blocks.WATER.defaultBlockState()});
        blockers.add(new Object[]{8, -59, 8, slab(SlabType.BOTTOM)});
        blockers.add(new Object[]{12, -60, 2, Blocks.TORCH.defaultBlockState()});
        blockers.add(new Object[]{12, -59, 2, slab(SlabType.TOP)});
        blockers.add(new Object[]{13, -60, 2, Blocks.OAK_STAIRS.defaultBlockState().setValue(StairBlock.FACING, Direction.EAST)});
        blockers.add(new Object[]{11, -60, 2, Blocks.OAK_STAIRS.defaultBlockState()});
        for (int x = 4; x <= 6; x++) {
            for (int z = 12; z <= 14; z++) {
                blockers.add(new Object[]{x, -58, z, slab(SlabType.BOTTOM)});
            }
        }
        blockers.add(new Object[]{5, -60, 13, Blocks.GLOWSTONE.defaultBlockState()});
        steps.add(blockers);
        List<Object[]> removals = new ArrayList<>();
        removals.add(new Object[]{8, -60, 8, Blocks.AIR.defaultBlockState()});
        for (int x = 13; x <= 15; x++) {
            for (int z = 6; z <= 10; z++) {
                removals.add(new Object[]{x, -57, z, Blocks.AIR.defaultBlockState()});
            }
        }
        removals.add(new Object[]{12, -59, 2, Blocks.AIR.defaultBlockState()});
        removals.add(new Object[]{5, -58, 13, Blocks.AIR.defaultBlockState()});
        steps.add(removals);
        return steps;
    }

    static final int[][] PROBES = {
        {8, -60, 8}, {8, -59, 8}, {8, -58, 8}, {10, -60, 8}, {8, -61, 8},
        {15, -58, 8}, {15, -60, 8}, {17, -60, 8}, {19, -60, 8}, {12, -60, 8},
        {3, -60, 3}, {3, -61, 3}, {5, -61, 3}, {7, -61, 3}, {7, -60, 3},
        {12, -60, 2}, {12, -59, 2}, {12, -58, 2}, {13, -60, 2}, {14, -60, 2}, {11, -60, 2}, {10, -60, 2},
        {5, -59, 13}, {5, -58, 13}, {5, -57, 13}, {4, -59, 12}, {7, -59, 13},
    };

    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        for (Block block : net.minecraft.core.registries.BuiltInRegistries.BLOCK) {
            block.getStateDefinition().getPossibleStates().forEach(BlockState::initCache);
        }
        Holder<Biome> plains = Holder.direct(null);
        IdMap<Holder<Biome>> biomes = new IdMap<>() {
            @Override
            public int getId(final Holder<Biome> thing) {
                return 0;
            }

            @Override
            public Holder<Biome> byId(final int id) {
                return id == 0 ? plains : null;
            }

            @Override
            public int size() {
                return 1;
            }

            @Override
            public Iterator<Holder<Biome>> iterator() {
                return List.of(plains).iterator();
            }
        };
        factory = new PalettedContainerFactory(
            Strategy.createForBlockStates(Block.BLOCK_STATE_REGISTRY), Blocks.AIR.defaultBlockState(), null, Strategy.createForBiomes(biomes), plains, null
        );

        LightOracle flat = new LightOracle(flat());
        flat.prepareTicking(0, 0);
        flat.dump("flat", -1, 1, -1, 1, new int[][]{{8, -60, 8}, {8, -61, 8}, {8, 300, 8}, {-8, -60, 20}});

        List<BlockState> layers = new ArrayList<>();
        layers.add(Blocks.BEDROCK.defaultBlockState());
        layers.add(Blocks.DIRT.defaultBlockState());
        layers.add(Blocks.DIRT.defaultBlockState());
        for (int i = 0; i < 4; i++) {
            layers.add(Blocks.WATER.defaultBlockState());
        }
        for (int i = 0; i < 3; i++) {
            layers.add(Blocks.OAK_LEAVES.defaultBlockState());
        }
        layers.add(Blocks.GLASS.defaultBlockState());
        LightOracle layered = new LightOracle(layers);
        layered.prepareTicking(0, 0);
        int[][] column = new int[11][];
        for (int i = 0; i < 11; i++) {
            column[i] = new int[]{8, -52 - i, 8};
        }
        layered.dump("layers", 0, 1, 0, 1, column);

        LightOracle incremental = new LightOracle(flat());
        incremental.prepareTicking(0, 0);
        incremental.prepareTicking(1, 0);
        List<List<Object[]>> steps = steps();
        for (int i = 0; i < steps.size(); i++) {
            for (Object[] change : steps.get(i)) {
                incremental.setBlock((Integer)change[0], (Integer)change[1], (Integer)change[2], (BlockState)change[3]);
            }
            incremental.runAll();
            incremental.dump("step " + (i + 1), -1, 2, -1, 1, PROBES);
        }

        LightOracle fromScratch = new LightOracle(flat());
        for (List<Object[]> step : steps) {
            for (Object[] change : step) {
                fromScratch.decorations.put(new BlockPos((Integer)change[0], (Integer)change[1], (Integer)change[2]), (BlockState)change[3]);
            }
        }
        fromScratch.prepareTicking(0, 0);
        fromScratch.prepareTicking(1, 0);
        fromScratch.dump("from scratch", -1, 2, -1, 1, PROBES);
    }
}
