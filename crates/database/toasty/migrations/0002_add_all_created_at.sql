-- Existing rows have no recorded creation time. Start their retention window
-- when this migration is applied, rather than deleting them immediately.
ALTER TABLE "fruit" ADD COLUMN "created_at" TIMESTAMPTZ(6);
UPDATE "fruit" SET "created_at" = CURRENT_TIMESTAMP WHERE "created_at" IS NULL;
ALTER TABLE "fruit" ALTER COLUMN "created_at" SET NOT NULL;
ALTER TABLE "cake_filling" ADD COLUMN "created_at" TIMESTAMPTZ(6);
UPDATE "cake_filling" SET "created_at" = CURRENT_TIMESTAMP WHERE "created_at" IS NULL;
ALTER TABLE "cake_filling" ALTER COLUMN "created_at" SET NOT NULL;
ALTER TABLE "filling" ADD COLUMN "created_at" TIMESTAMPTZ(6);
UPDATE "filling" SET "created_at" = CURRENT_TIMESTAMP WHERE "created_at" IS NULL;
ALTER TABLE "filling" ALTER COLUMN "created_at" SET NOT NULL;
