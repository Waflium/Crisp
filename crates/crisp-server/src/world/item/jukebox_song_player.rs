use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};

use crate::server::level::ServerLevel;
use crate::server::level::events::{SOUND_PLAY_JUKEBOX_SONG, SOUND_STOP_JUKEBOX_SONG};
use crate::world::item::jukebox_song::JukeboxSong;

pub const PLAY_EVENT_INTERVAL_TICKS: i64 = 20;

#[derive(Clone, Default)]
pub struct JukeboxSongPlayer {
    ticks_since_song_started: i64,
    song: Option<JukeboxSong>,
}

impl JukeboxSongPlayer {
    pub fn is_playing(&self) -> bool {
        self.song.is_some()
    }

    pub fn get_song(&self) -> Option<JukeboxSong> {
        self.song
    }

    pub fn get_ticks_since_song_started(&self) -> i64 {
        self.ticks_since_song_started
    }

    pub fn set_song_without_playing(&mut self, song: JukeboxSong, ticks_since_song_started: i64) {
        if !song.has_finished(ticks_since_song_started) {
            self.song = Some(song);
            self.ticks_since_song_started = ticks_since_song_started;
        }
    }

    pub fn clear_song(&mut self) {
        self.song = None;
        self.ticks_since_song_started = 0;
    }

    fn should_emit_jukebox_playing_event(&self) -> bool {
        self.ticks_since_song_started % PLAY_EVENT_INTERVAL_TICKS == 0
    }
}

impl ServerLevel {
    fn jukebox_song_player(&mut self, pos: BlockPos) -> Option<&mut JukeboxSongPlayer> {
        self.jukebox(pos).map(|jukebox| &mut jukebox.jukebox_song_player)
    }

    pub fn jukebox_song_player_play(&mut self, pos: BlockPos, song: JukeboxSong) {
        let Some(player) = self.jukebox_song_player(pos) else { return };
        player.song = Some(song);
        player.ticks_since_song_started = 0;
        self.level_event(None, SOUND_PLAY_JUKEBOX_SONG, pos, song.id);
        self.jukebox_on_song_changed(pos);
    }

    pub fn jukebox_song_player_stop(&mut self, pos: BlockPos) {
        let Some(player) = self.jukebox_song_player(pos) else { return };
        if player.song.is_some() {
            player.clear_song();
            self.level_event(None, SOUND_STOP_JUKEBOX_SONG, pos, 0);
            self.jukebox_on_song_changed(pos);
        }
    }

    pub fn jukebox_song_player_tick(&mut self, pos: BlockPos) {
        let Some(player) = self.jukebox_song_player(pos) else { return };
        let Some(song) = player.song else { return };
        if song.has_finished(player.ticks_since_song_started) {
            self.jukebox_song_player_stop(pos);
        } else {
            if player.should_emit_jukebox_playing_event() {
                self.spawn_music_particles(pos);
            }
            if let Some(player) = self.jukebox_song_player(pos) {
                player.ticks_since_song_started += 1;
            }
        }
    }

    fn spawn_music_particles(&mut self, block_pos: BlockPos) {
        let pos = Vec3::at_bottom_center_of(block_pos).add(0.0, 1.2f32 as f64, 0.0);
        let random_color = self.random.next_int_bounded(4) as f32 / 24.0;
        self.send_particles("minecraft:note", |_| {}, pos.x, pos.y, pos.z, 0, (random_color as f64, 0.0, 0.0), 1.0);
    }
}
