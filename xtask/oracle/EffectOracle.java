package crisp;

import com.mojang.serialization.Codec;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.nbt.NbtOps;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import net.minecraft.world.entity.ai.attributes.AttributeModifier;
import net.minecraft.world.entity.ai.attributes.Attributes;

public class EffectOracle {
   static String nbt(MobEffectInstance instance) {
      return MobEffectInstance.CODEC.encodeStart(NbtOps.INSTANCE, instance).getOrThrow().toString();
   }

   static void attribute(String label, Holder<Attribute> attribute, double base, Object[][] steps) {
      AttributeInstance instance = new AttributeInstance(attribute, i -> {});
      instance.setBaseValue(base);
      for (Object[] step : steps) {
         Identifier id = Identifier.parse((String) step[1]);
         switch ((String) step[0]) {
            case "transient" -> instance.addTransientModifier(new AttributeModifier(id, (Double) step[2], AttributeModifier.Operation.values()[(Integer) step[3]]));
            case "permanent" -> instance.addPermanentModifier(new AttributeModifier(id, (Double) step[2], AttributeModifier.Operation.values()[(Integer) step[3]]));
            case "remove" -> instance.removeModifier(id);
            default -> throw new IllegalArgumentException();
         }
      }
      double value = instance.getValue();
      String packed = AttributeInstance.Packed.CODEC.encodeStart(NbtOps.INSTANCE, instance.pack()).getOrThrow().toString();
      System.out.println("attr " + label + " " + Long.toHexString(Double.doubleToRawLongBits(value)) + " " + packed);
   }

   public static void main(String[] args) {
      SharedConstants.tryDetectVersion();
      Bootstrap.bootStrap();

      System.out.println("nbt plain " + nbt(new MobEffectInstance(MobEffects.SPEED, 600)));
      System.out.println("nbt amplified " + nbt(new MobEffectInstance(MobEffects.REGENERATION, 100, 1)));
      System.out.println("nbt infinite " + nbt(new MobEffectInstance(MobEffects.NIGHT_VISION, -1, 255, true, false, false)));
      System.out.println("nbt zero " + nbt(new MobEffectInstance(MobEffects.LUCK, 0, 0, false, true, false)));
      MobEffectInstance stacked = new MobEffectInstance(MobEffects.STRENGTH, 400, 0);
      boolean changed = stacked.update(new MobEffectInstance(MobEffects.STRENGTH, 100, 2));
      System.out.println("update stronger " + changed + " " + nbt(stacked));
      changed = stacked.update(new MobEffectInstance(MobEffects.STRENGTH, 200, 1));
      System.out.println("update hidden " + changed + " " + nbt(stacked));
      changed = stacked.update(new MobEffectInstance(MobEffects.STRENGTH, 50, 2, true, true));
      System.out.println("update weaker " + changed + " " + nbt(stacked));
      changed = stacked.update(new MobEffectInstance(MobEffects.STRENGTH, 300, 2, false, false, true));
      System.out.println("update longer " + changed + " " + nbt(stacked));
      for (int i = 0; i < 101; i++) {
         stacked.tickClient();
      }
      System.out.println("ticked " + nbt(stacked));
      for (int i = 0; i < 100; i++) {
         stacked.tickClient();
      }
      System.out.println("ticked " + nbt(stacked));
      MobEffectInstance infinite = new MobEffectInstance(MobEffects.HASTE, -1, 0);
      changed = infinite.update(new MobEffectInstance(MobEffects.HASTE, 1000, 0));
      System.out.println("update infinite " + changed + " " + nbt(infinite));
      changed = infinite.update(new MobEffectInstance(MobEffects.HASTE, 1000, 1));
      System.out.println("update over infinite " + changed + " " + nbt(infinite));

      List<Holder<MobEffect>> periodic = List.of(MobEffects.REGENERATION, MobEffects.POISON, MobEffects.WITHER, MobEffects.RAID_OMEN, MobEffects.INSTANT_HEALTH, MobEffects.SPEED);
      for (Holder<MobEffect> effect : periodic) {
         for (int amplifier : new int[] {0, 1, 2, 5, 6, 31, 32}) {
            List<Integer> ticks = new ArrayList<>();
            for (int duration = -1; duration <= 120; duration++) {
               if (effect.value().shouldApplyEffectTickThisTick(duration, amplifier)) {
                  ticks.add(duration);
               }
            }
            System.out.println("apply " + effect.getRegisteredName() + " " + amplifier + " " + ticks.size() + " " + ticks.subList(0, Math.min(8, ticks.size())));
         }
      }

      attribute("speed", Attributes.MOVEMENT_SPEED, 0.1F, new Object[][] {
         {"transient", "minecraft:sprinting", (double) 0.3F, 2},
         {"permanent", "minecraft:effect.speed", 0.20000000298023224 * 3, 2},
         {"permanent", "minecraft:effect.slowness", -0.15000000596046448 * 2, 2},
      });
      List<Object[]> many = new ArrayList<>();
      for (int i = 0; i < 40; i++) {
         many.add(new Object[] {i % 3 == 0 ? "transient" : "permanent", "test:m" + i, 0.001 * i + 1.0E-12 * (i * 7 % 11), i % 3});
      }
      for (int i = 0; i < 40; i += 3) {
         many.add(new Object[] {"remove", "test:m" + i});
      }
      many.add(new Object[] {"permanent", "z:last", 0.3, 0});
      attribute("many", Attributes.LUCK, 1.0, many.toArray(new Object[0][]));
      attribute("sums", Attributes.ARMOR, 0.1, new Object[][] {
         {"permanent", "a", 0.1, 0},
         {"permanent", "b", 0.2, 0},
         {"permanent", "c", 1.0E16, 0},
         {"permanent", "d", -1.0E16, 0},
         {"permanent", "e", 0.3, 0},
         {"permanent", "f", 3.0E-17, 0},
      });
      attribute("clamped", Attributes.MAX_HEALTH, 20.0, new Object[][] {{"permanent", "big", 5000.0, 0}});
   }
}
