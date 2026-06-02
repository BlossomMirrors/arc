-- CreateTable
CREATE TABLE "FrontPage" (
    "id" TEXT NOT NULL DEFAULT 'singleton',
    "sections" JSONB NOT NULL DEFAULT '[]',
    "updatedAt" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "FrontPage_pkey" PRIMARY KEY ("id")
);
