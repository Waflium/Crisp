package crisp;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.ByteBufOutputStream;
import io.netty.buffer.Unpooled;
import java.io.DataOutputStream;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import net.minecraft.ChatFormatting;
import net.minecraft.SharedConstants;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.network.chat.Style;
import net.minecraft.network.chat.numbers.BlankFormat;
import net.minecraft.network.chat.numbers.FixedFormat;
import net.minecraft.network.chat.numbers.StyledFormat;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.game.ClientboundResetScorePacket;
import net.minecraft.network.protocol.game.ClientboundSetDisplayObjectivePacket;
import net.minecraft.network.protocol.game.ClientboundSetObjectivePacket;
import net.minecraft.network.protocol.game.ClientboundSetPlayerTeamPacket;
import net.minecraft.network.protocol.game.ClientboundSetScorePacket;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.scores.DisplaySlot;
import net.minecraft.world.scores.Objective;
import net.minecraft.world.scores.PlayerTeam;
import net.minecraft.world.scores.ScoreAccess;
import net.minecraft.world.scores.ScoreHolder;
import net.minecraft.world.scores.Scoreboard;
import net.minecraft.world.scores.ScoreboardSaveData;
import net.minecraft.world.scores.Team;
import net.minecraft.world.scores.TeamColor;
import net.minecraft.world.scores.criteria.ObjectiveCriteria;

public class ScoreboardOracle {
   static class Packing extends Scoreboard {
      ScoreboardSaveData.Packed store() {
         return new ScoreboardSaveData.Packed(this.packObjectives(), this.packPlayerScores(), this.packDisplaySlots(), this.packPlayerTeams());
      }
   }

   static RegistryAccess ACCESS;

   static String hex(ByteBuf buf) {
      StringBuilder out = new StringBuilder();
      while (buf.isReadable()) {
         out.append(String.format("%02x", buf.readByte()));
      }
      return out.toString();
   }

   static String componentHex(Component component) {
      RegistryFriendlyByteBuf buf = new RegistryFriendlyByteBuf(Unpooled.buffer(), ACCESS);
      ComponentSerialization.TRUSTED_STREAM_CODEC.encode(buf, component);
      return hex(buf);
   }

   static <T> void packet(String name, StreamCodec<? super RegistryFriendlyByteBuf, T> codec, T packet) {
      RegistryFriendlyByteBuf buf = new RegistryFriendlyByteBuf(Unpooled.buffer(), ACCESS);
      codec.encode(buf, packet);
      System.out.println(name + " " + hex(buf));
   }

   public static void main(String[] args) throws Exception {
      SharedConstants.tryDetectVersion();
      Bootstrap.bootStrap();
      ACCESS = RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);

      Scoreboard orders = new Scoreboard();
      List<Objective> objectives = new ArrayList<>();
      for (int i = 0; i < 40; i++) {
         String name = i % 2 == 0 ? "obj" + i : "o_" + (i * 7919);
         objectives.add(orders.addObjective(name, ObjectiveCriteria.DUMMY, Component.literal(name), ObjectiveCriteria.RenderType.INTEGER, false, null));
      }
      for (int i = 0; i < 40; i += 3) {
         orders.removeObjective(objectives.get(i));
      }
      System.out.println("objectives " + String.join(",", orders.getObjectives().stream().map(Objective::getName).toList()));
      List<String> iterated = new ArrayList<>();
      for (Objective o : orders.getObjectives()) {
         iterated.add(o.getName());
      }
      System.out.println("objectives_iterated " + String.join(",", iterated));

      Objective first = objectives.get(1);
      for (int i = 0; i < 70; i++) {
         orders.getOrCreatePlayerScore(ScoreHolder.forNameOnly(i % 5 == 0 ? "#fake" + i : "Player" + i), first).set(i);
      }
      for (int i = 0; i < 70; i++) {
         if (i % 7 != 3) {
            orders.resetAllPlayerScores(ScoreHolder.forNameOnly(i % 5 == 0 ? "#fake" + i : "Player" + i));
         }
      }
      System.out.println("tracked " + String.join(",", orders.getTrackedPlayers().stream().map(ScoreHolder::getScoreboardName).toList()));
      System.out.println("listed " + String.join(",", orders.listPlayerScores(first).stream().map(e -> e.owner()).toList()));

      PlayerTeam crowd = orders.addPlayerTeam("crowd");
      for (int i = 0; i < 30; i++) {
         orders.addPlayerToTeam("member" + (i * 31), crowd);
      }
      for (int i = 0; i < 30; i += 4) {
         orders.removePlayerFromTeam("member" + (i * 31), crowd);
      }
      System.out.println("members " + String.join(",", crowd.getPlayers()));
      for (String name : new String[]{"red", "blue", "green", "a", "zz", "Team1"}) {
         orders.addPlayerTeam(name);
      }
      List<String> teams = new ArrayList<>();
      for (PlayerTeam team : orders.getPlayerTeams()) {
         teams.add(team.getName());
      }
      System.out.println("teams " + String.join(",", teams));

      Scoreboard board = new Scoreboard();
      Objective foo = board.addObjective(
         "foo", ObjectiveCriteria.DUMMY, Component.literal("Foo").withStyle(ChatFormatting.RED), ObjectiveCriteria.RenderType.HEARTS, false,
         new StyledFormat(Style.EMPTY.withColor(ChatFormatting.GOLD).withBold(true))
      );
      Objective bar = board.addObjective("bar", ObjectiveCriteria.DEATH_COUNT, Component.literal("bar"), ObjectiveCriteria.RenderType.INTEGER, false, null);
      packet("set_objective_add", ClientboundSetObjectivePacket.STREAM_CODEC, new ClientboundSetObjectivePacket(foo, 0));
      packet("set_objective_remove", ClientboundSetObjectivePacket.STREAM_CODEC, new ClientboundSetObjectivePacket(foo, 1));
      bar.setNumberFormat(new FixedFormat(Component.literal("-")));
      packet("set_objective_change", ClientboundSetObjectivePacket.STREAM_CODEC, new ClientboundSetObjectivePacket(bar, 2));
      bar.setNumberFormat(BlankFormat.INSTANCE);
      packet("set_objective_blank", ClientboundSetObjectivePacket.STREAM_CODEC, new ClientboundSetObjectivePacket(bar, 2));
      packet("display_sidebar", ClientboundSetDisplayObjectivePacket.STREAM_CODEC, new ClientboundSetDisplayObjectivePacket(DisplaySlot.SIDEBAR, foo));
      packet("display_team_red_none", ClientboundSetDisplayObjectivePacket.STREAM_CODEC, new ClientboundSetDisplayObjectivePacket(DisplaySlot.TEAM_RED, null));
      packet("set_score", ClientboundSetScorePacket.STREAM_CODEC, new ClientboundSetScorePacket("alice", "foo", 5, Optional.empty(), Optional.empty()));
      packet(
         "set_score_display",
         ClientboundSetScorePacket.STREAM_CODEC,
         new ClientboundSetScorePacket("#x", "foo", -300, Optional.of(Component.literal("X").withStyle(ChatFormatting.AQUA)), Optional.of(BlankFormat.INSTANCE))
      );
      packet("reset_all", ClientboundResetScorePacket.STREAM_CODEC, new ClientboundResetScorePacket("alice", null));
      packet("reset_one", ClientboundResetScorePacket.STREAM_CODEC, new ClientboundResetScorePacket("alice", "foo"));
      PlayerTeam red = board.addPlayerTeam("red");
      red.setDisplayName(Component.literal("Reds"));
      red.setColor(Optional.of(TeamColor.RED));
      red.setPlayerPrefix(Component.literal("[R] "));
      red.setAllowFriendlyFire(false);
      red.setNameTagVisibility(Team.Visibility.HIDE_FOR_OTHER_TEAMS);
      red.setCollisionRule(Team.CollisionRule.PUSH_OWN_TEAM);
      board.addPlayerToTeam("alice", red);
      board.addPlayerToTeam("bob", red);
      packet("team_add", ClientboundSetPlayerTeamPacket.STREAM_CODEC, ClientboundSetPlayerTeamPacket.createAddOrModifyPacket(red, true));
      packet("team_modify", ClientboundSetPlayerTeamPacket.STREAM_CODEC, ClientboundSetPlayerTeamPacket.createAddOrModifyPacket(red, false));
      packet("team_remove", ClientboundSetPlayerTeamPacket.STREAM_CODEC, ClientboundSetPlayerTeamPacket.createRemovePacket(red));
      packet("team_join", ClientboundSetPlayerTeamPacket.STREAM_CODEC, ClientboundSetPlayerTeamPacket.createPlayerPacket(red, "carol", ClientboundSetPlayerTeamPacket.Action.ADD));
      packet("team_leave", ClientboundSetPlayerTeamPacket.STREAM_CODEC, ClientboundSetPlayerTeamPacket.createPlayerPacket(red, "bob", ClientboundSetPlayerTeamPacket.Action.REMOVE));
      System.out.println("formatted_team " + componentHex(red.getFormattedDisplayName()));
      System.out.println("formatted_name " + componentHex(red.getFormattedName(Component.literal("alice"))));
      System.out.println("formatted_objective " + componentHex(foo.getFormattedDisplayName()));

      Packing save = new Packing();
      Objective kills = save.addObjective("kills", ObjectiveCriteria.KILL_COUNT_ALL, Component.literal("Kills"), ObjectiveCriteria.RenderType.INTEGER, true, null);
      Objective plain = save.addObjective("plain", ObjectiveCriteria.DUMMY, Component.literal("plain"), ObjectiveCriteria.RenderType.INTEGER, false, null);
      Objective hearts = save.addObjective(
         "hp", ObjectiveCriteria.HEALTH, Component.literal("HP").withStyle(ChatFormatting.RED), ObjectiveCriteria.RenderType.HEARTS, false, new FixedFormat(Component.literal("?"))
      );
      save.addObjective("trig", ObjectiveCriteria.TRIGGER, Component.literal("trig"), ObjectiveCriteria.RenderType.INTEGER, false, StyledFormat.SIDEBAR_DEFAULT);
      save.getOrCreatePlayerScore(ScoreHolder.forNameOnly("alice"), kills).set(3);
      save.getOrCreatePlayerScore(ScoreHolder.forNameOnly("carol"), plain).set(0);
      ScoreAccess bob = save.getOrCreatePlayerScore(ScoreHolder.forNameOnly("bob"), plain);
      bob.set(-7);
      bob.unlock();
      bob.display(Component.literal("Bobby"));
      bob.numberFormatOverride(BlankFormat.INSTANCE);
      save.getOrCreatePlayerScore(ScoreHolder.forNameOnly("#global"), plain).set(42);
      save.setDisplayObjective(DisplaySlot.SIDEBAR, kills);
      save.setDisplayObjective(DisplaySlot.LIST, hearts);
      save.setDisplayObjective(DisplaySlot.TEAM_GOLD, plain);
      PlayerTeam blue = save.addPlayerTeam("blue");
      blue.setColor(Optional.of(TeamColor.BLUE));
      blue.setSeeFriendlyInvisibles(false);
      blue.setPlayerSuffix(Component.literal("!"));
      blue.setDeathMessageVisibility(Team.Visibility.NEVER);
      save.addPlayerToTeam("alice", blue);
      save.addPlayerToTeam("bob", blue);
      save.addPlayerTeam("empty");
      Tag tag = ScoreboardSaveData.Packed.CODEC.encodeStart(NbtOps.INSTANCE, save.store()).getOrThrow();
      ByteBuf bytes = Unpooled.buffer();
      NbtIo.writeAnyTag(tag, new DataOutputStream(new ByteBufOutputStream(bytes)));
      System.out.println("save " + hex(bytes));
   }
}
