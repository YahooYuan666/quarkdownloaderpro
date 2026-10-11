import React from 'react';
import { RefreshCw, HardDrive } from 'lucide-react';
import { cn, formatSize } from '../../utils';
import { useQuarkStore } from '../../store/useQuarkStore';
import { motion } from 'framer-motion';

interface CapacityStatsProps {
    onRefresh: () => void;
}

export const CapacityStats: React.FC<CapacityStatsProps> = ({ onRefresh }) => {
    const { capacity } = useQuarkStore();
    const capacityLoading = capacity.loading;

    return (
        <div className="bg-muted/50 rounded-3xl p-4 border border-border">
            <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2 text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
                    <HardDrive size={12} />
                    存储空间
                </div>
                <button
                    onClick={onRefresh}
                    disabled={capacityLoading}
                    className="text-muted-foreground hover:text-primary transition-colors"
                >
                    <RefreshCw size={12} className={cn(capacityLoading && "animate-spin")} />
                </button>
            </div>

            <div className="space-y-2">
                <div className="h-1.5 bg-muted rounded-full overflow-hidden">
                    <motion.div
                        initial={{ width: 0 }}
                        animate={{ width: `${capacity.total > 0 ? (capacity.used / capacity.total) * 100 : 0}%` }}
                        className="h-full gradient-bg rounded-full"
                    />
                </div>
                <div className="flex justify-between text-[10px] text-muted-foreground font-bold">
                    <span>{formatSize(capacity.used)}</span>
                    <span>{formatSize(capacity.total)}</span>
                </div>
            </div>
        </div>
    );
};
