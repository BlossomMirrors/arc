-- CreateTable
CREATE TABLE "PwaTranslation" (
    "id" TEXT NOT NULL,
    "appId" TEXT NOT NULL,
    "lang" TEXT NOT NULL,
    "name" TEXT,
    "summary" TEXT,
    "description" TEXT,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "PwaTranslation_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE UNIQUE INDEX "PwaTranslation_appId_lang_key" ON "PwaTranslation"("appId", "lang");

-- AddForeignKey
ALTER TABLE "PwaTranslation" ADD CONSTRAINT "PwaTranslation_appId_fkey" FOREIGN KEY ("appId") REFERENCES "PwaApp"("id") ON DELETE CASCADE ON UPDATE CASCADE;
