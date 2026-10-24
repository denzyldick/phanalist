<?php

namespace Vendor {
    class ImportedBase
    {
        public static function zero(): int
        {
            return 0;
        }
    }
}

namespace App {
    use Vendor\ImportedBase as Base;

    class ImportedChild extends Base
    {
        public static function useParent(): float
        {
            return 1 / parent::zero();
        }
    }
}
