-- CreateTable
CREATE TABLE "PwaApp" (
    "id" TEXT NOT NULL,
    "appid" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "summary" TEXT NOT NULL,
    "description" TEXT NOT NULL,
    "iconUrl" TEXT NOT NULL,
    "screenshots" TEXT[],
    "homepageUrl" TEXT NOT NULL DEFAULT '',
    "contentRating" TEXT NOT NULL DEFAULT 'All ages',
    "developerName" TEXT NOT NULL,
    "url" TEXT NOT NULL,
    "color" TEXT NOT NULL DEFAULT '#000000',
    "css" TEXT NOT NULL DEFAULT '',
    "js" TEXT NOT NULL DEFAULT '',
    "useragent" TEXT NOT NULL DEFAULT '',
    "widevine" BOOLEAN NOT NULL DEFAULT false,
    "tray" BOOLEAN NOT NULL DEFAULT false,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "PwaApp_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE UNIQUE INDEX "PwaApp_appid_key" ON "PwaApp"("appid");
