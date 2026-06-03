-- CreateTable
CREATE TABLE "AppInstall" (
    "id" TEXT NOT NULL,
    "appid" TEXT NOT NULL,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "AppInstall_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "FlathubCache" (
    "appid" TEXT NOT NULL,
    "installs" INTEGER NOT NULL,
    "cachedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "FlathubCache_pkey" PRIMARY KEY ("appid")
);

-- CreateIndex
CREATE INDEX "AppInstall_appid_idx" ON "AppInstall"("appid");

-- CreateIndex
CREATE INDEX "AppInstall_appid_createdAt_idx" ON "AppInstall"("appid", "createdAt");
