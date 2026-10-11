package crisp;

import java.io.PrintWriter;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.CloseableResourceManager;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.ScheduledTickAccess;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.SpeleothemBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.SpeleothemThickness;
import net.minecraft.world.ticks.LevelTickAccess;

public class SpeleothemOracle {
   static final Map<BlockPos, BlockState> states = new HashMap<>();
   static boolean scheduled;
   static int blockTickDelay;

   static List<BlockState> speleothems() {
      List<BlockState> out = new ArrayList<>();
      for (Block block : new Block[]{Blocks.POINTED_DRIPSTONE, Blocks.SULFUR_SPIKE}) {
         for (Direction direction : new Direction[]{Direction.UP, Direction.DOWN}) {
            for (SpeleothemThickness thickness : SpeleothemThickness.values()) {
               out.add(block.defaultBlockState().setValue(SpeleothemBlock.TIP_DIRECTION, direction).setValue(SpeleothemBlock.THICKNESS, thickness));
            }
         }
      }
      return out;
   }

   public static void main(String[] args) throws Exception {
      SharedConstants.tryDetectVersion();
      Bootstrap.bootStrap();
      PackRepository packs = ServerPacksSource.createVanillaTrustedRepository();
      packs.reload();
      packs.setSelected(List.of("vanilla"));
      CloseableResourceManager resources = new MultiPackResourceManager(PackType.SERVER_DATA, packs.openAllSelected());
      LayeredRegistryAccess<RegistryLayer> layers = RegistryLayer.createRegistryAccess();
      TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC)).forEach(Registry.PendingTags::apply);

      InvocationHandler levelHandler = (proxy, method, a) -> {
         switch (method.getName()) {
            case "getBlockState":
               return states.getOrDefault((BlockPos) a[0], Blocks.AIR.defaultBlockState());
            case "getFluidState":
               return states.getOrDefault((BlockPos) a[0], Blocks.AIR.defaultBlockState()).getFluidState();
            default:
               if (method.isDefault()) {
                  return InvocationHandler.invokeDefault(proxy, method, a);
               }
               throw new UnsupportedOperationException(method.toString());
         }
      };
      LevelReader level = (LevelReader) Proxy.newProxyInstance(LevelReader.class.getClassLoader(), new Class<?>[]{LevelReader.class}, levelHandler);
      @SuppressWarnings("unchecked")
      LevelTickAccess<Block> blockTicks = (LevelTickAccess<Block>) Proxy.newProxyInstance(
         LevelTickAccess.class.getClassLoader(), new Class<?>[]{LevelTickAccess.class}, (proxy, method, a) -> {
            if (method.getName().equals("hasScheduledTick")) {
               return scheduled;
            }
            throw new UnsupportedOperationException(method.toString());
         }
      );
      ScheduledTickAccess ticks = (ScheduledTickAccess) Proxy.newProxyInstance(
         ScheduledTickAccess.class.getClassLoader(), new Class<?>[]{ScheduledTickAccess.class}, (proxy, method, a) -> {
            switch (method.getName()) {
               case "getBlockTicks":
                  return blockTicks;
               case "scheduleTick":
                  if (a[1] instanceof Block) {
                     blockTickDelay = (Integer) a[2];
                  }
                  return null;
               default:
                  throw new UnsupportedOperationException(method.toString());
            }
         }
      );

      List<BlockState> centres = speleothems();
      List<BlockState> neighbours = new ArrayList<>();
      neighbours.add(Blocks.AIR.defaultBlockState());
      neighbours.add(Blocks.STONE.defaultBlockState());
      neighbours.add(Blocks.OAK_FENCE.defaultBlockState());
      neighbours.add(Blocks.WATER.defaultBlockState());
      neighbours.addAll(centres);

      BlockPos pos = BlockPos.ZERO;
      RandomSource random = RandomSource.create(0L);
      try (PrintWriter out = new PrintWriter(args[0])) {
         for (BlockState centre : centres) {
            for (BlockState below : neighbours) {
               StringBuilder line = new StringBuilder();
               for (BlockState above : neighbours) {
                  for (Direction direction : new Direction[]{Direction.UP, Direction.DOWN}) {
                     for (boolean hasTick : new boolean[]{false, true}) {
                        states.clear();
                        states.put(pos, centre);
                        states.put(pos.below(), below);
                        states.put(pos.above(), above);
                        scheduled = hasTick;
                        blockTickDelay = 0;
                        BlockPos neighbourPos = pos.relative(direction);
                        BlockState result = centre.updateShape(level, ticks, pos, direction, neighbourPos, states.get(neighbourPos), random);
                        int thickness = result.getValue(SpeleothemBlock.THICKNESS).ordinal();
                        char base = blockTickDelay == 0 ? 'a' : blockTickDelay == 1 ? 'A' : 'k';
                        line.append((char) (base + thickness));
                     }
                  }
               }
               out.println(line);
            }
         }
      }
      System.out.println("wrote " + centres.size() * neighbours.size() + " lines to " + args[0]);
   }
}
