package crisp;

import java.io.BufferedOutputStream;
import java.io.DataOutputStream;
import java.io.FileOutputStream;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Mirror;
import net.minecraft.world.level.block.Rotation;
import net.minecraft.world.level.block.state.BlockState;

public class RotationOracle {
   static void writeId(DataOutputStream out, BlockState state) throws Exception {
      int id = Block.getId(state);
      out.writeByte(id & 0xFF);
      out.writeByte(id >>> 8 & 0xFF);
   }

   public static void main(String[] args) throws Exception {
      SharedConstants.tryDetectVersion();
      Bootstrap.bootStrap();

      int count = Block.BLOCK_STATE_REGISTRY.size();
      try (DataOutputStream out = new DataOutputStream(new BufferedOutputStream(new FileOutputStream(args[0])))) {
         for (int id = 0; id < count; id++) {
            BlockState state = Block.BLOCK_STATE_REGISTRY.byId(id);
            for (Rotation rotation : Rotation.values()) {
               writeId(out, state.rotate(rotation));
            }
            for (Mirror mirror : Mirror.values()) {
               writeId(out, state.mirror(mirror));
            }
         }
      }
      System.out.println("wrote " + count + " states to " + args[0]);
   }
}
