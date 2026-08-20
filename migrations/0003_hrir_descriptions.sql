ALTER TABLE hrir_presets
    ADD COLUMN description TEXT;

ALTER TABLE hrir_presets
    ADD CONSTRAINT hrir_presets_description_valid CHECK (
        description IS NULL OR (
            char_length(description) BETWEEN 1 AND 240
            AND description = btrim(description)
            AND description !~ '[[:cntrl:]]'
        )
    );
