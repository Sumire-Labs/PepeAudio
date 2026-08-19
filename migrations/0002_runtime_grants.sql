-- Runtime services can mutate product data but cannot own or alter the schema.
GRANT USAGE ON SCHEMA public TO pepeaudio_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO pepeaudio_runtime;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO pepeaudio_runtime;

ALTER DEFAULT PRIVILEGES FOR ROLE pepeaudio_migrator IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO pepeaudio_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE pepeaudio_migrator IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO pepeaudio_runtime;
